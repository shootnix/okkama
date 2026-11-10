//! `blade install` / `remove` / `update` against real git repositories and
//! local package directories in a temporary tree.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use blade::install::{self, Unlock};
use blade::lock::Lock;
use blade::manifest::{self, Manifest};

/// A temporary tree removed when dropped.
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Tree {
        let root = std::env::temp_dir().join(format!("blade-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Tree(std::fs::canonicalize(&root).unwrap())
    }

    fn write(&self, path: &str, text: &str) -> PathBuf {
        let file = self.0.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, text).unwrap();
        file
    }

    fn path(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn manifest_text(name: &str, version: &str, dependencies: &str) -> String {
    format!(
        "[module]\nname = \"{name}\"\nversion = \"{version}\"\n\n[dependencies]\n{dependencies}"
    )
}

/// Commit the repository at `dir` with `version` in its blade.toml, a module
/// `src/<name>.ok` and `files`, and tag it.
fn release(dir: &Path, name: &str, version: &str, dependencies: &str, files: &[(&str, &str)]) {
    let manifest = manifest_text(name, version, dependencies);
    let module = format!("module {name};\n");
    let source = format!("src/{name}.ok");
    let mut all = vec![
        ("blade.toml", manifest.as_str()),
        (source.as_str(), module.as_str()),
    ];
    all.extend_from_slice(files);
    tag(dir, version, &all);
}

/// Commit `files` into the repository at `dir`, created if needed, and tag
/// the commit `v<version>`.
fn tag(dir: &Path, version: &str, files: &[(&str, &str)]) {
    if !dir.join(".git").exists() {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q"]);
    }
    for (file, text) in files {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", version]);
    git(dir, &["tag", &format!("v{version}")]);
}

fn codec(text: &str) -> (&'static str, String) {
    (
        "src/codec.ok",
        format!(
            "module json/codec;\nobject Codec {{ Codec(); name() is string {{ \"{text}\"; }} }}\n"
        ),
    )
}

/// `json` 1.0.0, 1.2.0 and `web` 12.3.0, 12.3.1, 13.0.0 (needs json ≥ 1.0.0); a project `app`.
fn world(name: &str) -> (Tree, PathBuf, PathBuf) {
    let tree = Tree::new(name);
    let json = tree.path("repos/json");
    for version in ["1.0.0", "1.2.0"] {
        let (file, text) = codec(&format!("json {version}"));
        release(&json, "json", version, "", &[(file, &text)]);
    }
    let web = tree.path("repos/web");
    let needs_json = format!(
        "json = {{ git = \"{}\", version = \"1.0.0\" }}\n",
        json.display()
    );
    for version in ["12.3.0", "12.3.1", "13.0.0"] {
        let router = format!(
            "module web/router;\nimport \"pkg/json/src/codec\" take Codec;\n\
             object Router {{ Router(); route() is string {{ \"web {version} \" + Codec().name(); }} }}\n"
        );
        release(
            &web,
            "web",
            version,
            &needs_json,
            &[("src/router.ok", &router)],
        );
    }
    tree.write("app/blade.toml", &manifest_text("app", "1.0.0", ""));
    let app = tree.path("app");
    (tree, app, web)
}

fn locked(app: &Path) -> Vec<(String, String)> {
    Lock::read(app)
        .unwrap()
        .packages
        .into_iter()
        .map(|(name, package)| (name, package.version.to_string()))
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(name, version)| (name.to_string(), version.to_string()))
        .collect()
}

#[test]
fn git_dependencies_take_the_lowest_version_that_meets_their_minimums_and_stay_locked() {
    let (tree, app, web) = world("git");
    let (name, report) = install::add(&app, web.to_str().unwrap(), Some("12.3.0"), true).unwrap();
    assert_eq!(name, "web");
    assert_eq!(report.lines, ["+ json 1.0.0", "+ web 12.3.0"]);
    assert_eq!(locked(&app), pairs(&[("json", "1.0.0"), ("web", "12.3.0")]));
    let router = std::fs::read_to_string(app.join("pkg/web/src/router.ok")).unwrap();
    assert!(router.contains("web 12.3.0"), "{router}");
    assert!(!app.join("pkg/web/.git").exists());
    assert_eq!(
        Manifest::read(&app).unwrap().dependencies["web"]
            .version
            .as_deref(),
        Some("12.3.0")
    );

    // A new release changes nothing, installed or updated: the minimums
    // still meet at the same versions.
    let (file, text) = codec("json 1.3.0");
    release(
        &tree.path("repos/json"),
        "json",
        "1.3.0",
        "",
        &[(file, &text)],
    );
    assert!(
        install::install(&app, &Unlock::Nothing)
            .unwrap()
            .lines
            .is_empty()
    );
    let update_json = Unlock::Packages(BTreeSet::from(["json".to_string()]));
    assert!(
        install::install(&app, &update_json)
            .unwrap()
            .lines
            .is_empty()
    );
    assert!(
        install::install(&app, &Unlock::All)
            .unwrap()
            .lines
            .is_empty()
    );
    assert_eq!(locked(&app), pairs(&[("json", "1.0.0"), ("web", "12.3.0")]));
    // Raising a minimum moves the package to the lowest version that meets it.
    let json = tree.path("repos/json");
    manifest::add_dependency(
        &app,
        "json",
        &manifest::Dependency {
            git: Some(json.display().to_string()),
            version: Some("1.2.0".to_string()),
            path: None,
        },
    )
    .unwrap();
    let report = install::install(&app, &Unlock::Nothing).unwrap();
    assert_eq!(report.lines, ["~ json 1.0.0 -> 1.2.0"]);
}

/// Minimum version selection (#347): the lowest version that meets every
/// minimum; the requirements of the version selected, not only of the first
/// one seen; cycles; requirements of a superseded version that conflict or
/// that the selected version drops; a new tag; majors and sources that
/// cannot meet.
#[test]
fn minimums_meet_at_the_lowest_version_that_satisfies_them_all() {
    let tree = Tree::new("mvs");
    let repo = |name: &str| tree.path(&format!("repos/{name}"));
    let dep = |name: &str, min: &str| {
        format!(
            "{name} = {{ git = \"{}\", version = \"{min}\" }}\n",
            repo(name).display()
        )
    };
    release(&repo("x"), "x", "1.0.0", "", &[]);
    for version in ["1.0.0", "2.3.0", "2.4.0"] {
        release(&repo("util"), "util", version, "", &[]);
    }
    // json 1.2.0 needs util 1.x and x; 1.3.0 and 1.4.0 need util ≥ 2.3.0.
    release(
        &repo("json"),
        "json",
        "1.2.0",
        &(dep("util", "1.0.0") + &dep("x", "1.0.0")),
        &[],
    );
    for version in ["1.3.0", "1.4.0"] {
        release(&repo("json"), "json", version, &dep("util", "2.3.0"), &[]);
    }
    release(&repo("json"), "json", "2.0.0", "", &[]);
    release(&repo("http"), "http", "2.1.0", &dep("json", "1.2.0"), &[]);
    // database and back need each other.
    release(
        &repo("database"),
        "database",
        "3.0.0",
        &(dep("json", "1.4.0") + &dep("back", "1.0.0")),
        &[],
    );
    release(
        &repo("back"),
        "back",
        "1.0.0",
        &dep("database", "3.0.0"),
        &[],
    );
    release(&repo("next"), "next", "1.0.0", &dep("json", "2.0.0"), &[]);
    let project = |name: &str, dependencies: &str| {
        tree.write(
            &format!("{name}/blade.toml"),
            &manifest_text(name, "1.0.0", dependencies),
        );
        tree.path(name)
    };

    let alone = project("alone", &dep("http", "2.1.0"));
    install::install(&alone, &Unlock::Nothing).unwrap();
    assert_eq!(
        locked(&alone),
        pairs(&[
            ("http", "2.1.0"),
            ("json", "1.2.0"),
            ("util", "1.0.0"),
            ("x", "1.0.0")
        ])
    );

    // json ≥ 1.2.0 and ≥ 1.4.0 meet at 1.4.0, whose own requirement brings
    // util 2.3.0; util 1.x and x, wanted only by json 1.2.0, are gone, and
    // the cycle of database and back ends.
    let both = project("both", &(dep("http", "2.1.0") + &dep("database", "3.0.0")));
    install::install(&both, &Unlock::Nothing).unwrap();
    let expected = pairs(&[
        ("back", "1.0.0"),
        ("database", "3.0.0"),
        ("http", "2.1.0"),
        ("json", "1.4.0"),
        ("util", "2.3.0"),
    ]);
    assert_eq!(locked(&both), expected);
    assert!(!both.join("pkg/x").exists());

    // A newer tag does not change a fresh resolution.
    release(&repo("json"), "json", "1.5.0", &dep("util", "2.3.0"), &[]);
    let again = project("again", &(dep("database", "3.0.0") + &dep("http", "2.1.0")));
    install::install(&again, &Unlock::Nothing).unwrap();
    assert_eq!(locked(&again), expected);

    let majors = project("majors", &(dep("http", "2.1.0") + &dep("next", "1.0.0")));
    let error = install::install(&majors, &Unlock::Nothing).unwrap_err();
    assert!(error.contains("no single major of \"json\""), "{error}");
    assert!(
        error.contains("http 2.1.0 → json ≥ 1.2.0") && error.contains("next 1.0.0 → json ≥ 2.0.0"),
        "{error}"
    );
    assert!(!majors.join("pkg").exists() && !majors.join("blade.lock").exists());

    release(&repo("fork"), "json", "1.2.0", "", &[]);
    let forked = format!(
        "json = {{ git = \"{}\", version = \"1.2.0\" }}\n",
        repo("fork").display()
    );
    release(&repo("forker"), "forker", "1.0.0", &forked, &[]);
    let sources = project("sources", &(dep("http", "2.1.0") + &dep("forker", "1.0.0")));
    let error = install::install(&sources, &Unlock::Nothing).unwrap_err();
    assert!(error.contains("\"json\" comes from two sources"), "{error}");

    // A superseded version may need what cannot be had: here, a repository
    // that does not exist.
    release(&repo("old"), "old", "1.0.0", &dep("ghost", "1.0.0"), &[]);
    release(&repo("old"), "old", "1.1.0", "", &[]);
    release(&repo("early"), "early", "1.0.0", &dep("old", "1.0.0"), &[]);
    release(&repo("late"), "late", "1.0.0", &dep("old", "1.1.0"), &[]);
    let superseded = project(
        "superseded",
        &(dep("early", "1.0.0") + &dep("late", "1.0.0")),
    );
    install::install(&superseded, &Unlock::Nothing).unwrap();
    assert_eq!(
        locked(&superseded),
        pairs(&[("early", "1.0.0"), ("late", "1.0.0"), ("old", "1.1.0")])
    );
    let error = install::install(
        &project("ghostly", &dep("early", "1.0.0")),
        &Unlock::Nothing,
    )
    .unwrap_err();
    assert!(error.contains("ghost"), "{error}");

    let missing = project("missing", &dep("json", "1.9.0"));
    let error = install::install(&missing, &Unlock::Nothing).unwrap_err();
    assert!(
        error.contains("no version of \"json\" is ≥ 1.9.0 within major 1"),
        "{error}"
    );
}

#[test]
fn a_moved_tag_is_refused_until_update() {
    let (tree, app, web) = world("moved");
    install::add(&app, web.to_str().unwrap(), Some("13.0.0"), true).unwrap();
    // Move the tag json 1.0.0 to a new commit of the same release.
    let json = tree.path("repos/json");
    git(&json, &["checkout", "-q", "v1.0.0"]);
    std::fs::write(json.join("src/codec.ok"), codec("changed").1).unwrap();
    git(&json, &["commit", "-qam", "changed"]);
    git(&json, &["tag", "-f", "v1.0.0"]);
    git(&json, &["checkout", "-q", "main"]);
    let error = install::install(&app, &Unlock::Nothing).unwrap_err();
    assert!(
        error.contains("tag v1.0.0 of") && error.contains("the tag was moved"),
        "{error}"
    );
    let report = install::install(&app, &Unlock::All).unwrap();
    assert_eq!(report.lines.len(), 1, "{:?}", report.lines);
    assert!(
        report.lines[0].starts_with("~ json 1.0.0 (commit "),
        "{:?}",
        report.lines
    );
    assert!(
        install::install(&app, &Unlock::Nothing)
            .unwrap()
            .lines
            .is_empty()
    );
}

#[test]
fn local_directories_install_by_their_version_and_remove_cleanly() {
    let (tree, app, _) = world("path");
    let util = tree.write("local/util/blade.toml", &manifest_text("util", "2.1.0", ""));
    tree.write(
        "local/util/src/util.ok",
        "module util;\nobject Shout { Shout(); }\n",
    );
    let dir = util.parent().unwrap().to_str().unwrap().to_string();
    let error = install::add(&app, &dir, Some("3.0.0"), false).unwrap_err();
    assert!(
        error.contains("has no version matching ≥ 3.0.0; available: 2.1.0"),
        "{error}"
    );
    let (name, report) = install::add(&app, &dir, None, false).unwrap();
    assert_eq!(
        (name.as_str(), report.lines),
        ("util", vec!["+ util 2.1.0".to_string()])
    );
    assert!(app.join("pkg/util/src/util.ok").is_file());

    // Правка локального пакета видна после следующего install.
    tree.write(
        "local/util/src/extra.ok",
        "module util/extra;\nobject Extra { Extra(); }\n",
    );
    install::install(&app, &Unlock::Nothing).unwrap();
    assert!(app.join("pkg/util/src/extra.ok").is_file());

    assert!(manifest::remove_dependency(&app, "util").unwrap());
    let report = install::install(&app, &Unlock::Nothing).unwrap();
    assert_eq!(report.lines, ["- util 2.1.0"]);
    assert!(!app.join("pkg/util").exists());
    assert!(Lock::read(&app).unwrap().packages.is_empty());
}

#[test]
fn bad_packages_and_conflicts_leave_the_project_as_it_was() {
    let (tree, app, web) = world("bad");
    install::add(&app, web.to_str().unwrap(), Some("13.0.0"), true).unwrap();
    let manifest = std::fs::read_to_string(app.join("blade.toml")).unwrap();
    let lock = std::fs::read_to_string(app.join("blade.lock")).unwrap();

    let json = tree.path("repos/json");
    let package = |name: &str, dependencies: &str, files: &[(&str, &str)]| {
        let file = tree.write(
            &format!("local/{name}/blade.toml"),
            &manifest_text(name, "1.0.0", dependencies),
        );
        tree.write(
            &format!("local/{name}/src/{name}.ok"),
            &format!("module {name};\n"),
        );
        for (path, text) in files {
            tree.write(&format!("local/{name}/{path}"), text);
        }
        file.parent().unwrap().to_str().unwrap().to_string()
    };
    let cases = [
        (
            package(
                "other",
                &format!(
                    "json = {{ git = \"{}\", version = \"2.0.0\" }}\n",
                    json.display()
                ),
                &[],
            ),
            "no single major of \"json\"",
        ),
        (
            package(
                "nat",
                "",
                &[("src/nat.ok", "module nat;\nnative now() is int;\n")],
            ),
            "src/nat.ok declares a native",
        ),
        (
            package(
                "ver",
                "",
                &[("src/ver.ok", "module ver v1.0.0;\nobject X { X(); }\n")],
            ),
            "src/ver.ok declares module version v1.0.0",
        ),
        (
            package(
                "pre",
                "",
                &[("src/x.ok", "module util;\nobject X { X(); }\n")],
            ),
            "src/x.ok declares module \"util\"; modules of package \"pre\" are named \"pre\" or \"pre/…\"",
        ),
    ];
    for (dir, expected) in cases {
        let error = install::add(&app, &dir, None, false).unwrap_err();
        assert!(error.contains(expected), "{error}\nexpected: {expected}");
        assert_eq!(
            std::fs::read_to_string(app.join("blade.toml")).unwrap(),
            manifest
        );
        assert_eq!(
            std::fs::read_to_string(app.join("blade.lock")).unwrap(),
            lock
        );
        assert!(app.join("pkg/web/src/router.ok").is_file());
        assert!(!app.join("pkg.blade-busy").exists());
    }

    let link = package("lnk", "", &[]);
    #[cfg(unix)]
    std::os::unix::fs::symlink("/etc/hosts", Path::new(&link).join("hosts.ok")).unwrap();
    #[cfg(unix)]
    {
        let error = install::add(&app, &link, None, false).unwrap_err();
        assert!(error.contains("is a symbolic link"), "{error}");
    }

    // Имя в [dependencies] должно совпадать с [module] name пакета.
    let renamed = package("renamed", "", &[]);
    manifest::add_dependency(
        &app,
        "alias",
        &manifest::Dependency {
            path: Some(renamed),
            version: Some("1.0.0".to_string()),
            git: None,
        },
    )
    .unwrap();
    let error = install::install(&app, &Unlock::Nothing).unwrap_err();
    assert!(
        error.contains("alias 1.0.0 (path+")
            && error.contains(
                "declares [module] name \"renamed\"; the name in [dependencies] must match"
            ),
        "{error}"
    );
}

/// The package format (#348): a git repository with its modules in `src/`,
/// named by the key it is declared under, versioned by its tag, with a
/// `blade.toml` only for dependencies; a generated one is installed when it
/// has none. Only `src/` is checked.
#[test]
fn a_package_is_a_repository_with_its_modules_in_src() {
    let tree = Tree::new("format");
    let repo = |name: &str| tree.path(&format!("repos/{name}"));
    let dep = |name: &str, min: &str| {
        format!(
            "{name} = {{ git = \"{}\", version = \"{min}\" }}\n",
            repo(name).display()
        )
    };
    let project = |name: &str, dependencies: &str| {
        tree.write(
            &format!("{name}/blade.toml"),
            &manifest_text(name, "1.0.0", dependencies),
        );
        tree.path(name)
    };

    // Only src/json.ok; the examples and tests beside it would fail the checks.
    tag(
        &repo("json"),
        "1.0.0",
        &[
            ("src/json.ok", "module json;\nobject Codec { Codec(); }\n"),
            ("LICENSE", "MIT\n"),
            ("examples/broken.ok", "this is not okkama {"),
            ("tests/native.ok", "module other;\nnative now() is int;\n"),
        ],
    );
    let app = project("app", "");
    let (name, report) = install::add(&app, repo("json").to_str().unwrap(), None, true).unwrap();
    assert_eq!(
        (name.as_str(), report.lines),
        ("json", vec!["+ json 1.0.0".to_string()])
    );
    let generated = "[module]\nname = \"json\"\nversion = \"1.0.0\"\n";
    assert_eq!(
        std::fs::read_to_string(app.join("pkg/json/blade.toml")).unwrap(),
        generated
    );
    for file in ["src/json.ok", "LICENSE", "examples/broken.ok"] {
        assert!(app.join("pkg/json").join(file).is_file(), "{file}");
    }
    // The same bytes and the same checksum in another project.
    let again = project("again", &dep("json", "1.0.0"));
    install::install(&again, &Unlock::Nothing).unwrap();
    assert_eq!(
        std::fs::read(again.join("pkg/json/blade.toml")).unwrap(),
        generated.as_bytes()
    );
    let checksum = |app: &Path| Lock::read(app).unwrap().packages["json"].checksum.clone();
    assert_eq!(checksum(&app), checksum(&again));

    // A library whose blade.toml has only [dependencies] keeps it.
    let web_manifest = format!("[dependencies]\n{}", dep("json", "1.0.0"));
    let router = "module web/router;\nimport \"pkg/json/src/json\" take Codec;\nobject Router { Router(); }\n";
    tag(
        &repo("web"),
        "2.0.0",
        &[("blade.toml", &web_manifest), ("src/router.ok", router)],
    );
    let site = project("site", &dep("web", "2.0.0"));
    install::install(&site, &Unlock::Nothing).unwrap();
    assert_eq!(locked(&site), pairs(&[("json", "1.0.0"), ("web", "2.0.0")]));
    let installed = Manifest::read(&site.join("pkg/web")).unwrap();
    assert!(installed.name.is_none() && installed.dependencies.contains_key("json"));

    // `blade install SOURCE` takes [module] name over the repository's name.
    tag(
        &repo("okkama-yaml"),
        "0.1.0",
        &[
            ("blade.toml", "[module]\nname = \"yaml\"\n"),
            ("src/yaml.ok", "module yaml;\n"),
        ],
    );
    let (name, _) = install::add(&app, repo("okkama-yaml").to_str().unwrap(), None, true).unwrap();
    assert_eq!(name, "yaml");
    assert!(app.join("pkg/yaml/src/yaml.ok").is_file());

    // Each refusal names the package and its tag and changes nothing.
    let refused = |name: &str, expected: &str| {
        let bad = project(&format!("uses_{name}"), &dep(name, "1.0.0"));
        let error = install::install(&bad, &Unlock::Nothing).unwrap_err();
        assert!(error.contains(&format!("{name} v1.0.0 (git+")), "{error}");
        assert!(error.contains(expected), "{error}\nexpected: {expected}");
        assert!(!bad.join("pkg").exists() && !bad.join("blade.lock").exists());
    };
    tag(
        &repo("named"),
        "1.0.0",
        &[
            ("blade.toml", "[module]\nname = \"other\"\n"),
            ("src/named.ok", "module named;\n"),
        ],
    );
    refused(
        "named",
        "declares [module] name \"other\"; the name in [dependencies] must match",
    );
    tag(
        &repo("versioned"),
        "1.0.0",
        &[
            ("blade.toml", "[module]\nversion = \"1.1.0\"\n"),
            ("src/versioned.ok", "module versioned;\n"),
        ],
    );
    refused(
        "versioned",
        "declares [module] version 1.1.0, but the tag is v1.0.0",
    );
    tag(&repo("flat"), "1.0.0", &[("flat.ok", "module flat;\n")]);
    refused("flat", "no src/ directory");
    tag(
        &repo("natives"),
        "1.0.0",
        &[("src/natives.ok", "module natives;\nnative now() is int;\n")],
    );
    refused("natives", "src/natives.ok declares a native");
    #[cfg(unix)]
    {
        let linked = repo("linked");
        std::fs::create_dir_all(linked.join("docs")).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", linked.join("docs/hosts")).unwrap();
        tag(&linked, "1.0.0", &[("src/linked.ok", "module linked;\n")]);
        refused("linked", "is a symbolic link");
    }
}

/// Every file under `dir` with its bytes, sorted.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let name = path.strip_prefix(dir).unwrap().display().to_string();
                out.push((name, std::fs::read(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// `blade sync` (#349): without a lock, the first install; then exactly what
/// the lock records, by commit, whatever the tags say now.
#[test]
fn sync_installs_exactly_what_the_lock_records() {
    let (tree, app, web) = world("sync");
    let declare = |min: &str| {
        let dependency = manifest::Dependency {
            git: Some(web.display().to_string()),
            version: Some(min.to_string()),
            path: None,
        };
        manifest::add_dependency(&app, "web", &dependency).unwrap();
    };
    declare("12.3.0");
    let report = install::sync(&app).unwrap();
    assert_eq!(report.lines, ["+ json 1.0.0", "+ web 12.3.0"]);
    assert!(install::sync(&app).unwrap().lines.is_empty());
    let (pkg, lock) = (
        snapshot(&app.join("pkg")),
        std::fs::read(app.join("blade.lock")).unwrap(),
    );

    // Restored from the locked commits, with one tag deleted and one moved.
    let json = tree.path("repos/json");
    git(&json, &["tag", "-d", "v1.0.0"]);
    git(&web, &["checkout", "-q", "v12.3.0"]);
    std::fs::write(web.join("src/router.ok"), "module web/router;\n").unwrap();
    git(&web, &["commit", "-qam", "moved"]);
    git(&web, &["tag", "-f", "v12.3.0"]);
    git(&web, &["checkout", "-q", "main"]);
    std::fs::remove_dir_all(app.join("pkg")).unwrap();
    let report = install::sync(&app).unwrap();
    assert_eq!(report.lines, ["+ json 1.0.0", "+ web 12.3.0"]);
    assert_eq!(snapshot(&app.join("pkg")), pkg);
    assert_eq!(std::fs::read(app.join("blade.lock")).unwrap(), lock);

    // What the lock does not have leaves pkg/.
    tree.write("app/pkg/stale/src/stale.ok", "module stale;\n");
    let report = install::sync(&app).unwrap();
    assert_eq!(report.lines, ["- stale"]);
    assert_eq!(snapshot(&app.join("pkg")), pkg);

    // A locked commit that is gone is an error that names it.
    let commit = Lock::read(&app).unwrap().packages["json"]
        .commit
        .clone()
        .unwrap();
    std::fs::remove_dir_all(&json).unwrap();
    for version in ["1.0.0", "1.2.0"] {
        let (file, text) = codec(&format!("json {version}, again"));
        release(&json, "json", version, "", &[(file, &text)]);
    }
    std::fs::remove_dir_all(app.join("pkg")).unwrap();
    let error = install::sync(&app).unwrap_err();
    assert!(
        error.contains(&format!("json 1.0.0: the locked commit {commit} is not in")),
        "{error}"
    );
    assert!(!app.join("pkg").exists());
    assert_eq!(std::fs::read(app.join("blade.lock")).unwrap(), lock);
}

/// A lock that disagrees with the manifests: each disagreement has its line,
/// and nothing changes (#349).
#[test]
fn a_lock_that_disagrees_with_the_manifests_changes_nothing() {
    let (tree, app, web) = world("disagree");
    install::add(&app, web.to_str().unwrap(), Some("12.3.0"), true).unwrap();
    let before = (
        std::fs::read_to_string(app.join("blade.toml")).unwrap(),
        std::fs::read(app.join("blade.lock")).unwrap(),
        snapshot(&app.join("pkg")),
    );
    tree.write("local/util/blade.toml", &manifest_text("util", "2.1.0", ""));
    tree.write("local/util/src/util.ok", "module util;\n");
    git(
        &tree.path("repos"),
        &["clone", "-q", web.to_str().unwrap(), "fork-web"],
    );
    let edit = |name: &str, git: Option<PathBuf>, path: Option<&str>, version: &str| {
        let dependency = manifest::Dependency {
            git: git.map(|url| url.display().to_string()),
            version: Some(version.to_string()),
            path: path.map(str::to_string),
        };
        manifest::add_dependency(&app, name, &dependency).unwrap();
    };
    let fork = tree.path("repos/fork-web");
    let cases: [(&dyn Fn(), &[&str]); 4] = [
        (
            &|| edit("web", Some(web.clone()), None, "12.3.1"),
            &["web: blade.toml wants ≥ 12.3.1, blade.lock has 12.3.0 — run `blade update web`"],
        ),
        (
            &|| edit("util", None, Some("../local/util"), "2.1.0"),
            &["util: blade.toml wants ≥ 2.1.0, blade.lock has none — run `blade update util`"],
        ),
        (
            &|| {
                manifest::remove_dependency(&app, "web").unwrap();
            },
            &[
                "json: no longer required — run `blade update`",
                "web: no longer required — run `blade update`",
            ],
        ),
        (
            &|| edit("web", Some(fork.clone()), None, "12.3.0"),
            &["web: blade.toml wants it from git+"],
        ),
    ];
    for (change, expected) in cases {
        change();
        let error = install::sync(&app).unwrap_err();
        assert!(
            error.starts_with("blade.lock does not match blade.toml:\n"),
            "{error}"
        );
        for line in expected {
            assert!(error.contains(line), "{error}\nexpected: {line}");
        }
        assert_eq!(
            (
                std::fs::read(app.join("blade.lock")).unwrap(),
                snapshot(&app.join("pkg"))
            ),
            (before.1.clone(), before.2.clone())
        );
        std::fs::write(app.join("blade.toml"), &before.0).unwrap();
    }
    assert!(install::sync(&app).unwrap().lines.is_empty());
}

#[test]
fn an_interrupted_sync_recovers_to_one_consistent_state() {
    for step in ["staged", "lock", "marker", "pkg-moved", "pkg-in", "lock-in"] {
        let (_tree, app, web) = world(&format!("sync-{step}"));
        install::add(&app, web.to_str().unwrap(), Some("12.3.0"), true).unwrap();
        let pkg = snapshot(&app.join("pkg"));
        std::fs::remove_dir_all(app.join("pkg")).unwrap();
        install::fail_after(Some(step));
        let error = install::sync(&app).unwrap_err();
        assert!(
            error.contains(&format!("interrupted after {step}")),
            "{error}"
        );
        install::sync(&app).unwrap();
        install::settled(&app).unwrap();
        assert_eq!(snapshot(&app.join("pkg")), pkg, "{step}");
        assert!(install::sync(&app).unwrap().lines.is_empty(), "{step}");
    }
}

/// S01 (#15): a package name is a module path, never a file system path.
#[test]
fn package_names_cannot_leave_the_project() {
    let (tree, app, _) = world("names");
    let victim = tree.write("victim/proof.txt", "original");
    // A package without `.ok` files skipped the namespace check: its name was
    // joined to the staging directory as a path.
    let evil = tree.write(
        "evil/blade.toml",
        &manifest_text("../../victim", "1.0.0", ""),
    );
    tree.write("evil/proof.txt", "replaced");
    let dir = evil.parent().unwrap().to_str().unwrap().to_string();
    let error = install::add(&app, &dir, None, false).unwrap_err();
    assert!(error.contains("package name"), "{error}");
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "original");
    assert!(!app.join("pkg").exists() || !app.join("pkg/../../victim").exists());
    assert!(!tree.path("victim/blade.toml").exists());

    for bad in [
        "/abs", "a//b", "", ".", "..", "a/../b", "a/./b", "a b", "1x", "a/", "/", "a\\b", "x:y",
    ] {
        let text = manifest_text(bad, "1.0.0", "");
        let error = Manifest::parse(&text).unwrap_err();
        assert!(error.contains("package name"), "{bad:?}: {error}");
        let text = manifest_text("app", "1.0.0", &format!("{:?} = \"1.0.0\"\n", bad));
        let error = Manifest::parse(&text).unwrap_err();
        assert!(error.contains("package name"), "{bad:?}: {error}");
    }
    for good in ["web", "json_ext", "acme/web", "_x2"] {
        Manifest::parse(&manifest_text(good, "1.0.0", "")).unwrap();
    }
    // Two dependencies cannot share a directory of pkg/ through letter case;
    // nested names (`acme`, `acme/web`) are allowed (the longest name wins).
    Manifest::parse(&manifest_text(
        "app",
        "1.0.0",
        "acme = \"1.0.0\"\n\"acme/web\" = \"1.0.0\"\n",
    ))
    .unwrap();
    {
        let (a, b) = ("Web", "web");
        let text = manifest_text(
            "app",
            "1.0.0",
            &format!("{a:?} = \"1.0.0\"\n{b:?} = \"1.0.0\"\n"),
        );
        let error = Manifest::parse(&text).unwrap_err();
        assert!(error.contains("share a directory"), "{a} {b}: {error}");
    }
}

/// The version of each installed package as its `pkg/` copy declares it.
fn installed(app: &Path) -> Vec<(String, String)> {
    Lock::read(app)
        .unwrap()
        .packages
        .keys()
        .map(|name| {
            let manifest = Manifest::read(&app.join("pkg").join(name)).unwrap();
            (name.clone(), manifest.version.unwrap())
        })
        .collect()
}

#[test]
fn an_interrupted_install_recovers_to_one_consistent_state() {
    for (step, done) in [
        ("staged", false),
        ("lock", false),
        ("marker", true),
        ("pkg-moved", true),
        ("pkg-in", true),
        ("lock-in", true),
    ] {
        let (tree, app, _) = world(&format!("interrupt-{step}"));
        let util = tree.write("local/util/blade.toml", &manifest_text("util", "2.1.0", ""));
        tree.write(
            "local/util/src/util.ok",
            "module util;\nobject Shout { Shout(); }\n",
        );
        let dir = util.parent().unwrap().to_str().unwrap().to_string();
        install::add(&app, &dir, Some("2.1.0"), false).unwrap();
        tree.write("local/util/blade.toml", &manifest_text("util", "2.2.0", ""));

        install::fail_after(Some(step));
        let error = install::install(&app, &Unlock::All).unwrap_err();
        assert!(
            error.contains(&format!("interrupted after {step}")),
            "{error}"
        );
        assert!(
            install::settled(&app).is_err(),
            "{step}: the interruption is visible"
        );

        install::recover(&app).unwrap();
        install::settled(&app).unwrap();
        let expected = if done { "2.2.0" } else { "2.1.0" };
        assert_eq!(locked(&app), pairs(&[("util", expected)]), "{step}");
        assert_eq!(installed(&app), pairs(&[("util", expected)]), "{step}");
        for leftover in [
            "pkg.blade-new",
            "pkg.blade-old",
            "blade.lock.blade-new",
            "pkg.blade-commit",
        ] {
            assert!(!app.join(leftover).exists(), "{step}: {leftover}");
        }
        // The next install starts from that state.
        install::install(&app, &Unlock::All).unwrap();
        assert_eq!(installed(&app), pairs(&[("util", "2.2.0")]));
    }
}

#[test]
fn a_busy_project_is_not_changed() {
    let (tree, app, _) = world("busy");
    let util = tree.write("local/util/blade.toml", &manifest_text("util", "2.1.0", ""));
    tree.write(
        "local/util/src/util.ok",
        "module util;\nobject Shout { Shout(); }\n",
    );
    let dir = util.parent().unwrap().to_str().unwrap().to_string();
    std::fs::write(app.join("pkg.blade-busy"), "").unwrap();
    let error = install::add(&app, &dir, None, false).unwrap_err();
    assert!(
        error.contains("another blade is working on this project"),
        "{error}"
    );
    assert!(Manifest::read(&app).unwrap().dependencies.is_empty());
    assert!(!app.join("pkg").exists() && !app.join("blade.lock").exists());
}

/// A git source is never an option or a transport helper (#346): a
/// dependency declared with one is refused before git runs, and nothing it
/// names is executed.
#[test]
fn git_sources_cannot_inject_options_or_transports() {
    let (tree, app, _) = world("inject");
    let marker = tree.path("INJECTED");
    for url in [
        format!("--upload-pack=touch {}; git-upload-pack", marker.display()),
        format!("-c core.sshCommand=touch {}", marker.display()),
        format!("ext::sh -c touch% {}", marker.display()),
        "https://example.org/json\n--upload-pack=x".to_string(),
    ] {
        let text = manifest_text(
            "app",
            "1.0.0",
            &format!("evil = {{ git = {url:?}, version = \"1.0.0\" }}\n"),
        );
        std::fs::write(app.join("blade.toml"), &text).unwrap();
        let error = install::install(&app, &Unlock::Nothing).unwrap_err();
        assert!(error.contains("invalid git source"), "{url:?}: {error}");
        let error = install::add(&app, &url, None, true).unwrap_err();
        assert!(error.contains("invalid git source"), "{url:?}: {error}");
        assert!(!marker.exists(), "{url:?} ran a command");
        assert!(
            !app.join("pkg").exists() && !app.join("blade.lock").exists(),
            "{url:?}"
        );
    }
}
