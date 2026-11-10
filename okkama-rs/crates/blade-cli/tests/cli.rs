//! `blade build` and `blade run` through the binary, with a stand-in `okkama`
//! (`BLADE_OKKAMA`) that records its arguments: no compiler is needed.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A temporary tree removed when dropped.
struct Tree(PathBuf);

impl Tree {
    fn new(name: &str) -> Tree {
        let root = std::env::temp_dir().join(format!("blade-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let tree = Tree(std::fs::canonicalize(&root).unwrap());
        // Stand-in okkama: logs its arguments; `build … -o OUT` writes OUT
        // (fails when the entry says FAIL); `exec` prints and exits 7.
        let fake = tree.0.join("okkama");
        std::fs::write(
            &fake,
            r#"#!/bin/sh
echo "$@" >> "$(dirname "$0")/okkama.log"
case "$1" in
build)
    grep -q FAIL "$2" && { echo "compile error" >&2; exit 1; }
    for last; do :; done
    echo built > "$last" ;;
exec|run)
    echo "ran $* greeting=$GREETING"
    exit 7 ;;
esac
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        tree
    }

    fn write(&self, path: &str, text: &str) {
        let file = self.0.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }

    fn blade(&self, dir: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_blade"))
            .args(args)
            .current_dir(self.0.join(dir))
            .env("BLADE_OKKAMA", self.0.join("okkama"))
            .output()
            .unwrap()
    }

    /// `blade` with only a stand-in `git` on `PATH`, which leaves a mark.
    fn blade_without_git(&self, dir: &str, args: &[&str]) -> Output {
        let bin = self.0.join("no-git");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("git"), "#!/bin/sh\ntouch \"$(dirname \"$0\")/ran\"\nexit 1\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin.join("git"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_blade"))
            .args(args)
            .current_dir(self.0.join(dir))
            .env("PATH", &bin)
            .output()
            .unwrap();
        assert!(!bin.join("ran").exists(), "git ran: {}", text(&output.stderr));
        output
    }

    /// The okkama calls so far, one per line; the log starts empty again.
    fn calls(&self) -> Vec<String> {
        let log = self.0.join("okkama.log");
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_file(&log);
        text.lines().map(|line| line.replace(&format!("{}/", self.0.display()), "")).collect()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Move the modification time of `file` back an hour.
fn age(file: &Path) {
    let time = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(file).unwrap().set_modified(time).unwrap();
}

#[test]
fn a_new_project_builds_and_runs_with_no_flags() {
    let tree = Tree::new("new");
    assert!(tree.blade(".", &["new", "app"]).status.success());
    let built = tree.blade("app", &["build"]);
    assert!(built.status.success(), "{}", text(&built.stderr));
    assert_eq!(text(&built.stdout), "built target/app.spb\n");
    assert_eq!(tree.calls(), ["build src/main.ok --target sparrow -o target/app.spb"]);

    // Up to date: run does not build; the program's status is Blade's.
    age(&tree.0.join("app/src/main.ok"));
    age(&tree.0.join("app/blade.toml"));
    let ran = tree.blade("app", &["run", "--", "a", "--help"]);
    assert_eq!(ran.status.code(), Some(7), "{}", text(&ran.stderr));
    assert_eq!(tree.calls(), ["exec app/target/app.spb a --help"]);

    // A changed source is built again before running.
    tree.write("app/src/extra.ok", "module app;\n");
    assert_eq!(tree.blade("app", &["run"]).status.code(), Some(7));
    assert_eq!(tree.calls(), ["build src/main.ok --target sparrow -o target/app.spb", "exec app/target/app.spb"]);
}

#[test]
fn blade_toml_decides_what_builds_and_runs() {
    let tree = Tree::new("settings");
    tree.write("p/main.ok", "module p;\n");
    tree.write("p/more.ok", "module p;\n");
    tree.write(
        "p/blade.toml",
        "[module]\nname = \"p\"\n\n[build]\nentry = [\"main.ok\", \"more.ok\"]\ntarget = \"jvm\"\noutput = \"out/p.jar\"\n\n\
         [run]\nargs = [\"x\"]\nenv = { GREETING = \"hi\" }\n",
    );
    let built = tree.blade("p", &["build"]);
    assert!(built.status.success(), "{}", text(&built.stderr));
    assert_eq!(tree.calls(), ["build main.ok more.ok --target jvm -o out/p.jar"]);
    assert!(tree.0.join("p/out/p.jar").is_file());

    tree.write(
        "p/blade.toml",
        "[module]\nname = \"p\"\n\n[build]\nentry = \"main.ok\"\n\n[run]\nengine = \"ir\"\nargs = [\"x\"]\nenv = { GREETING = \"hi\" }\n",
    );
    let ran = tree.blade("p", &["run", "--", "y"]);
    assert_eq!(ran.status.code(), Some(7));
    assert_eq!(text(&ran.stdout), "ran run --engine ir main.ok x y greeting=hi\n");
    // IR runs the sources: nothing is built.
    assert_eq!(tree.calls(), ["run --engine ir main.ok x y"]);
}

#[test]
fn errors_name_the_problem_and_fail() {
    let tree = Tree::new("errors");
    let cases = [
        ("[build]\nentrance = \"main.ok\"\n", "unknown key \"entrance\" in [build]"),
        ("[run]\nengine = \"llvm\"\n", "[run] engine \"llvm\" is not sparrow, ir or jvm"),
        ("[build]\ntarget = \"jvm\"\n[run]\nengine = \"sparrow\"\n", "engine \"sparrow\" needs [build] target \"sparrow\""),
        ("[build]\nentry = \"missing.ok\"\n", "[build] entry \"missing.ok\" does not exist"),
        ("[build]\nentry = \"bad.ok\"\n", "build failed"),
        ("[build]\nentry = \"main.ok\"\n[dependencies]\nweb = { path = \"../web\" }\n", "dependency \"web\" is not installed"),
    ];
    tree.write("e/main.ok", "module e;\n");
    tree.write("e/bad.ok", "module e; FAIL\n");
    for (settings, expected) in cases {
        tree.write("e/blade.toml", &format!("[module]\nname = \"e\"\n{settings}"));
        for command in ["build", "run"] {
            let output = tree.blade("e", &[command]);
            assert_eq!(output.status.code(), Some(2), "{command} {settings}");
            assert!(text(&output.stderr).contains(expected), "{command}: {}", text(&output.stderr));
        }
    }
    // Installed (locked and in pkg/), the dependency builds with the project.
    tree.write(
        "e/blade.lock",
        "version = 1\n\n[[package]]\nname = \"web\"\nversion = \"1.0.0\"\nsource = \"path+/web\"\nchecksum = \"sha256:00\"\n",
    );
    tree.write("e/pkg/web/router.ok", "module web/router;\n");
    let built = tree.blade("e", &["build"]);
    assert!(built.status.success(), "{}", text(&built.stderr));

    let output = tree.blade(".", &["build"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("no blade.toml"), "{}", text(&output.stderr));
}

/// `blade sync` starts no git when `pkg/` holds what `blade.lock` records
/// (#349): a vendored `pkg/`, and the second sync after the first install.
#[test]
fn sync_runs_no_git_when_pkg_matches_the_lock() {
    let tree = Tree::new("sync");
    tree.write("json/src/json.ok", "module json;\n");
    let json = tree.0.join("json");
    for args in [&["init", "-q"][..], &["add", "-A"], &["commit", "-qm", "1.0.0"], &["tag", "v1.0.0"]] {
        let done = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(&json)
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", text(&done.stderr));
    }
    tree.write(
        "app/blade.toml",
        &format!("[module]\nname = \"app\"\n\n[dependencies]\njson = {{ git = \"{}\", version = \"1.0.0\" }}\n", json.display()),
    );
    let first = tree.blade("app", &["sync"]);
    assert!(first.status.success(), "{}", text(&first.stderr));
    assert_eq!(text(&first.stdout), "+ json 1.0.0\n");
    let again = tree.blade_without_git("app", &["sync"]);
    assert!(again.status.success(), "{}", text(&again.stderr));
    assert_eq!(text(&again.stdout), "dependencies are up to date\n");

    // Committed with the project, pkg/ syncs elsewhere without git.
    let copy = Command::new("cp").args(["-R", "app", "vendored"]).current_dir(&tree.0).output().unwrap();
    assert!(copy.status.success());
    std::fs::remove_dir_all(&json).unwrap();
    let vendored = tree.blade_without_git("vendored", &["sync"]);
    assert!(vendored.status.success(), "{}", text(&vendored.stderr));
    assert_eq!(text(&vendored.stdout), "dependencies are up to date\n");
}

/// The walk-through of #351 through the binary: a new project, a package
/// that brings another, a build, a higher minimum, an update, a removal and
/// a sync; then the old spellings, the help and the exit codes.
#[test]
fn a_project_walks_through_add_update_remove_and_sync() {
    let tree = Tree::new("walk");
    let git = |dir: &Path, args: &[&str]| {
        let done = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(done.status.success(), "git {args:?}: {}", text(&done.stderr));
    };
    let repo = |name: &str| tree.0.join("repos").join(name);
    let release = |name: &str, version: &str, dependencies: &str| {
        tree.write(&format!("repos/{name}/src/{name}.ok"), &format!("module {name}; // {version}\n"));
        if !dependencies.is_empty() {
            tree.write(&format!("repos/{name}/blade.toml"), &format!("[dependencies]\n{dependencies}"));
        }
        if !repo(name).join(".git").exists() {
            git(&repo(name), &["init", "-q"]);
        }
        git(&repo(name), &["add", "-A"]);
        git(&repo(name), &["commit", "-qm", version]);
        git(&repo(name), &["tag", &format!("v{version}")]);
    };
    for version in ["1.2.0", "1.3.0", "1.4.0"] {
        release("json", version, "");
    }
    let json = repo("json").display().to_string();
    release("http", "2.1.0", &format!("json = {{ git = \"{json}\", version = \"1.2.0\" }}\n"));
    let http = repo("http").display().to_string();
    let blade = |args: &[&str]| {
        let output = tree.blade("app", args);
        assert!(output.status.success(), "blade {args:?}: {}", text(&output.stderr));
        text(&output.stdout)
    };
    let locked = || -> Vec<(String, String)> {
        blade::lock::Lock::read(&tree.0.join("app"))
            .unwrap()
            .packages
            .into_iter()
            .map(|(name, package)| (name, package.version.to_string()))
            .collect()
    };
    let pair = |name: &str, version: &str| (name.to_string(), version.to_string());
    let manifest = || std::fs::read_to_string(tree.0.join("app/blade.toml")).unwrap();
    let installed = |name: &str| tree.0.join("app/pkg").join(name).join("src").is_dir();

    assert!(tree.blade(".", &["new", "app"]).status.success());
    assert_eq!(blade(&["add", "http", &http]), "+ http 2.1.0\n+ json 1.2.0\n");
    assert!(manifest().contains(&format!("http = {{ git = \"{http}\", version = \"2.1.0\" }}")), "{}", manifest());
    assert_eq!(locked(), [pair("http", "2.1.0"), pair("json", "1.2.0")]);
    assert!(installed("http") && installed("json"));
    assert_eq!(blade(&["build"]), "built target/app.spb\n");
    assert_eq!(blade(&["add", "json", &json, "1.4.0"]), "~ json 1.2.0 -> 1.4.0\n");
    assert!(manifest().contains(&format!("json = {{ git = \"{json}\", version = \"1.4.0\" }}")), "{}", manifest());
    assert_eq!(blade(&["update"]), "dependencies are up to date\n");
    assert_eq!(blade(&["remove", "http"]), "- http 2.1.0\n");
    assert!(!manifest().contains("http ="), "{}", manifest());
    assert_eq!(locked(), [pair("json", "1.4.0")]);
    assert!(!installed("http") && installed("json"));
    assert_eq!(blade(&["sync"]), "dependencies are up to date\n");

    // The old spellings say what replaced them, and still work.
    let old = tree.blade("app", &["install"]);
    assert!(old.status.success());
    assert_eq!(text(&old.stderr), "blade: `blade install` is now `blade sync`\n");
    assert_eq!(text(&old.stdout), "dependencies are up to date\n");
    let old = tree.blade("app", &["install", "--git", &http, "2.1.0"]);
    assert!(old.status.success(), "{}", text(&old.stderr));
    assert_eq!(
        text(&old.stderr),
        format!("blade: `blade install SOURCE` is now `blade add http {http} 2.1.0`\n")
    );
    assert_eq!(text(&old.stdout), "+ http 2.1.0\n");

    let help = tree.blade(".", &["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(text(&help.stdout).contains("blade add NAME GIT [MIN]"));
    for wrong in [&["add"][..], &["add", "json"], &["add", "a", "b", "c", "d"], &["sync", "x"], &["bogus"]] {
        let output = tree.blade("app", wrong);
        assert_eq!(output.status.code(), Some(2), "{wrong:?}");
        assert!(text(&output.stderr).contains("USAGE:"), "{wrong:?}");
    }
}
