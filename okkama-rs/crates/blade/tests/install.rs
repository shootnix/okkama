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

/// Commit the repository at `dir` with `version` in its blade.toml and tag it.
fn release(dir: &Path, name: &str, version: &str, dependencies: &str, files: &[(&str, &str)]) {
    if !dir.join(".git").exists() {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q"]);
    }
    std::fs::write(
        dir.join("blade.toml"),
        manifest_text(name, version, dependencies),
    )
    .unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", version]);
    git(dir, &["tag", &format!("v{version}")]);
}

fn codec(text: &str) -> (&'static str, String) {
    (
        "codec.ok",
        format!(
            "module json/codec;\nobject Codec {{ Codec(); name() is string {{ \"{text}\"; }} }}\n"
        ),
    )
}

/// `json` 1.0.0, 1.2.0 and `web` 12.3.0, 12.3.1, 13.0.0 (needs json 1.*); a project `app`.
fn world(name: &str) -> (Tree, PathBuf, PathBuf) {
    let tree = Tree::new(name);
    let json = tree.path("repos/json");
    for version in ["1.0.0", "1.2.0"] {
        let (file, text) = codec(&format!("json {version}"));
        release(&json, "json", version, "", &[(file, &text)]);
    }
    let web = tree.path("repos/web");
    let needs_json = format!(
        "json = {{ git = \"{}\", version = \"1.*\" }}\n",
        json.display()
    );
    for version in ["12.3.0", "12.3.1", "13.0.0"] {
        let router = format!(
            "module web/router;\nimport \"pkg/json/codec\" take Codec;\n\
             object Router {{ Router(); route() is string {{ \"web {version} \" + Codec().name(); }} }}\n"
        );
        release(&web, "web", version, &needs_json, &[("router.ok", &router)]);
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
fn git_dependencies_take_the_newest_compatible_version_and_stay_locked() {
    let (tree, app, web) = world("git");
    let (name, report) = install::add(&app, web.to_str().unwrap(), Some("12.3.0"), true).unwrap();
    assert_eq!(name, "web");
    assert_eq!(report.lines, ["+ json 1.2.0", "+ web 12.3.1"]);
    assert_eq!(locked(&app), pairs(&[("json", "1.2.0"), ("web", "12.3.1")]));
    let router = std::fs::read_to_string(app.join("pkg/web/router.ok")).unwrap();
    assert!(router.contains("web 12.3.1"), "{router}");
    assert!(!app.join("pkg/web/.git").exists());
    assert_eq!(
        Manifest::read(&app).unwrap().dependencies["web"]
            .version
            .as_deref(),
        Some("12.3.0")
    );

    // Новая версия json не ставится сама: lock держит 1.2.0 до `update`.
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
    assert_eq!(locked(&app), pairs(&[("json", "1.2.0"), ("web", "12.3.1")]));
    let report = install::install(
        &app,
        &Unlock::Packages(BTreeSet::from(["json".to_string()])),
    )
    .unwrap();
    assert_eq!(report.lines, ["~ json 1.2.0 -> 1.3.0"]);
    let report = install::install(&app, &Unlock::All).unwrap();
    assert!(
        report.lines.is_empty(),
        "web stays within 12.*: {:?}",
        report.lines
    );
}

#[test]
fn a_moved_tag_is_refused_until_update() {
    let (tree, app, web) = world("moved");
    install::add(&app, web.to_str().unwrap(), Some("13.*"), true).unwrap();
    let json = tree.path("repos/json");
    std::fs::write(json.join("codec.ok"), codec("changed").1).unwrap();
    git(&json, &["commit", "-qam", "changed"]);
    git(&json, &["tag", "-f", "v1.2.0"]);
    let error = install::install(&app, &Unlock::Nothing).unwrap_err();
    assert!(
        error.contains("tag v1.2.0 of") && error.contains("the tag was moved"),
        "{error}"
    );
    let report = install::install(&app, &Unlock::All).unwrap();
    assert_eq!(report.lines.len(), 1, "{:?}", report.lines);
    assert!(
        report.lines[0].starts_with("~ json 1.2.0 (commit "),
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
        "local/util/util.ok",
        "module util;\nobject Shout { Shout(); }\n",
    );
    let dir = util.parent().unwrap().to_str().unwrap().to_string();
    let error = install::add(&app, &dir, Some("3.*"), false).unwrap_err();
    assert!(
        error.contains("has no version matching 3.*; available: 2.1.0"),
        "{error}"
    );
    let (name, report) = install::add(&app, &dir, None, false).unwrap();
    assert_eq!(
        (name.as_str(), report.lines),
        ("util", vec!["+ util 2.1.0".to_string()])
    );
    assert!(app.join("pkg/util/util.ok").is_file());

    // Правка локального пакета видна после следующего install.
    tree.write(
        "local/util/extra.ok",
        "module util/extra;\nobject Extra { Extra(); }\n",
    );
    install::install(&app, &Unlock::Nothing).unwrap();
    assert!(app.join("pkg/util/extra.ok").is_file());

    assert!(manifest::remove_dependency(&app, "util").unwrap());
    let report = install::install(&app, &Unlock::Nothing).unwrap();
    assert_eq!(report.lines, ["- util 2.1.0"]);
    assert!(!app.join("pkg/util").exists());
    assert!(Lock::read(&app).unwrap().packages.is_empty());
}

#[test]
fn bad_packages_and_conflicts_leave_the_project_as_it_was() {
    let (tree, app, web) = world("bad");
    install::add(&app, web.to_str().unwrap(), Some("13.*"), true).unwrap();
    let manifest = std::fs::read_to_string(app.join("blade.toml")).unwrap();
    let lock = std::fs::read_to_string(app.join("blade.lock")).unwrap();

    let json = tree.path("repos/json");
    let package = |name: &str, dependencies: &str, files: &[(&str, &str)]| {
        let file = tree.write(
            &format!("local/{name}/blade.toml"),
            &manifest_text(name, "1.0.0", dependencies),
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
                    "json = {{ git = \"{}\", version = \"2.*\" }}\n",
                    json.display()
                ),
                &[],
            ),
            "no version of \"json\" satisfies 1.* (by web 13.0.0), 2.* (by other 1.0.0); available: 1.0.0, 1.2.0",
        ),
        (
            package(
                "nat",
                "",
                &[("nat.ok", "module nat;\nnative now() is int;\n")],
            ),
            "nat.ok declares a native",
        ),
        (
            package(
                "ver",
                "",
                &[("ver.ok", "module ver v1.0.0;\nobject X { X(); }\n")],
            ),
            "ver.ok declares module version v1.0.0",
        ),
        (
            package("pre", "", &[("x.ok", "module util;\nobject X { X(); }\n")]),
            "x.ok declares module \"util\"; modules of package \"pre\" are named \"pre\" or \"pre/…\"",
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
        assert!(app.join("pkg/web/router.ok").is_file());
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
        error.contains("dependency \"alias\" is the package \"renamed\""),
        "{error}"
    );
}

/// S01 (#15): a package name is a module path, never a file system path.
#[test]
fn package_names_cannot_leave_the_project() {
    let (tree, app, _) = world("names");
    let victim = tree.write("victim/proof.txt", "original");
    // A package without `.ok` files skipped the namespace check: its name was
    // joined to the staging directory as a path.
    let evil = tree.write("evil/blade.toml", &manifest_text("../../victim", "1.0.0", ""));
    tree.write("evil/proof.txt", "replaced");
    let dir = evil.parent().unwrap().to_str().unwrap().to_string();
    let error = install::add(&app, &dir, None, false).unwrap_err();
    assert!(error.contains("package name"), "{error}");
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "original");
    assert!(!app.join("pkg").exists() || !app.join("pkg/../../victim").exists());
    assert!(!tree.path("victim/blade.toml").exists());

    for bad in ["/abs", "a//b", "", ".", "..", "a/../b", "a/./b", "a b", "1x", "a/", "/", "a\\b", "x:y"] {
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
    Manifest::parse(&manifest_text("app", "1.0.0", "acme = \"1.0.0\"\n\"acme/web\" = \"1.0.0\"\n")).unwrap();
    {
        let (a, b) = ("Web", "web");
        let text = manifest_text("app", "1.0.0", &format!("{a:?} = \"1.0.0\"\n{b:?} = \"1.0.0\"\n"));
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
        tree.write("local/util/util.ok", "module util;\nobject Shout { Shout(); }\n");
        let dir = util.parent().unwrap().to_str().unwrap().to_string();
        install::add(&app, &dir, Some("2.*"), false).unwrap();
        tree.write("local/util/blade.toml", &manifest_text("util", "2.2.0", ""));

        install::fail_after(Some(step));
        let error = install::install(&app, &Unlock::All).unwrap_err();
        assert!(error.contains(&format!("interrupted after {step}")), "{error}");
        assert!(install::settled(&app).is_err(), "{step}: the interruption is visible");

        install::recover(&app).unwrap();
        install::settled(&app).unwrap();
        let expected = if done { "2.2.0" } else { "2.1.0" };
        assert_eq!(locked(&app), pairs(&[("util", expected)]), "{step}");
        assert_eq!(installed(&app), pairs(&[("util", expected)]), "{step}");
        for leftover in ["pkg.blade-new", "pkg.blade-old", "blade.lock.blade-new", "pkg.blade-commit"] {
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
    tree.write("local/util/util.ok", "module util;\nobject Shout { Shout(); }\n");
    let dir = util.parent().unwrap().to_str().unwrap().to_string();
    std::fs::write(app.join("pkg.blade-busy"), "").unwrap();
    let error = install::add(&app, &dir, None, false).unwrap_err();
    assert!(error.contains("another blade is working on this project"), "{error}");
    assert!(Manifest::read(&app).unwrap().dependencies.is_empty());
    assert!(!app.join("pkg").exists() && !app.join("blade.lock").exists());
}
