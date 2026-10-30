//! Core library for the Blade command-line tool.

pub mod install;
pub mod lock;
pub mod manifest;
pub mod version;

use std::{env::Args, io::Write, process::ExitCode};
use std::path::PathBuf;
//use std::path::{Path, PathBuf};

use install::{Report, Unlock};

/// Help text shared by Blade command-line front ends.
pub const HELP: &str = "\
blade — Blade command-line tool

USAGE:
    blade new NAME                   create a project NAME
    blade install                    install the dependencies of blade.toml into pkg/
    blade install SOURCE [VERSION]   add a dependency and install it; SOURCE is a
                                     package directory or a git repository
                                     (--git treats a local directory as git)
    blade remove NAME                remove a dependency
    blade update [NAME...]           install the newest allowed versions
    blade --help
    blade --version

VERSION is 1.2.3 (compatible with 1.2.3), 1.*, 1.2.*, =1.2.3 or *.
";

/// Version of the Blade package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub trait Command {
    fn execute(&self) -> ExitCode;
}

#[derive(Debug)]
pub struct Cli {
    args: Args,
    //args: Vec<String>,
}

struct CommandNew {
    args: Vec<String>,
}

struct CommandInstall {
    args: Vec<String>,
}

struct CommandRemove {
    args: Vec<String>,
}

struct CommandUpdate {
    args: Vec<String>,
}

struct CommandUsage {
    //args: Args,
    error: bool,
}

struct CommandVersion {
    version: String,
}

impl Command for CommandNew {
    fn execute(&self) -> ExitCode {
        if let Err(e) = self.create_dir() {
            eprintln!("cannot make a root dir: {}", e);
            return ExitCode::from(2);
        }

        if let Err(e) = self.create_dir_internal(String::from("src")) {
            eprintln!("cannot make src dir: {}", e);
            return ExitCode::from(2);
        }
        
        if let Err(e) = self.create_dir_internal(String::from("bin")) {
            eprintln!("cannot make bin dir: {}", e);
            return ExitCode::from(2);
        }

        if let Err(e) = self.create_dir_internal(String::from(manifest::PKG)) {
            eprintln!("cannot make {} dir: {}", manifest::PKG, e);
            return ExitCode::from(2);
        }

        let project_name = self.args.last().unwrap();
        
        // create src/main.ok
        /*
            module hello;

            import "io/stdout" take Stdout;

            main() is int {
                Stdout.writeln("hello from okkama");
                0;
            }
        */
        let main_ok = format!(
    r#"module {project_name};
import "io/stdout" take Stdout;

main() is int {{

    Stdout().writeln("hello from okkama");
    0;
}}
"#

);
        if let Err(e) = self.create_file(String::from("src/main.ok"), main_ok.as_bytes()) {
            eprintln!("cannot make file src/main.ok: {}", e);
            return ExitCode::from(2);
        }

        

        // create blade.toml
        /*
            [module]
            name = "hello"
            version = "1.0.0"
            
            [build]
            target = "sparrow"
            output = "hello"

            [dependencies]
        */
        let hello_toml = format!(
r#"[module]
name = "{project_name}"
version = "1.0.0"

[build]
target = "sparrow"
output = "{project_name}"

[dependencies]
"#
);
        if let Err(e) = self.create_file(String::from(manifest::MANIFEST), hello_toml.as_bytes()) {
            eprintln!("cannot make file {}: {}", manifest::MANIFEST, e);
            return ExitCode::from(2);
        }

        ExitCode::SUCCESS
    }
}

impl Command for CommandInstall {
    fn execute(&self) -> ExitCode {
        let git = self.args.iter().any(|arg| arg == "--git");
        let rest: Vec<&String> = self.args.iter().filter(|arg| *arg != "--git").collect();
        let root = match project_root() {
            Ok(root) => root,
            Err(e) => return failure(&e),
        };
        let result = match rest.as_slice() {
            [] if !git => install::install(&root, &Unlock::Nothing),
            [source] => install::add(&root, source, None, git).map(|(_, report)| report),
            [source, version] => install::add(&root, source, Some(version), git).map(|(_, report)| report),
            _ => {
                eprint!("{HELP}");
                return ExitCode::from(2);
            }
        };
        finish(result)
    }
}

impl Command for CommandRemove {
    fn execute(&self) -> ExitCode {
        let [name] = self.args.as_slice() else {
            eprint!("{HELP}");
            return ExitCode::from(2);
        };
        let root = match project_root() {
            Ok(root) => root,
            Err(e) => return failure(&e),
        };
        match manifest::remove_dependency(&root, name) {
            Ok(true) => finish(install::install(&root, &Unlock::Nothing)),
            Ok(false) => failure(&format!("\"{name}\" is not a dependency in {}", manifest::MANIFEST)),
            Err(e) => failure(&e),
        }
    }
}

impl Command for CommandUpdate {
    fn execute(&self) -> ExitCode {
        let root = match project_root() {
            Ok(root) => root,
            Err(e) => return failure(&e),
        };
        let unlock = if self.args.is_empty() {
            Unlock::All
        } else {
            Unlock::Packages(self.args.iter().cloned().collect())
        };
        finish(install::install(&root, &unlock))
    }
}

/// The project of the current directory: the nearest directory with blade.toml.
fn project_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    manifest::find_root(&cwd)
        .ok_or_else(|| format!("no {} in {} or above it", manifest::MANIFEST, cwd.display()))
}

fn failure(message: &str) -> ExitCode {
    eprintln!("blade: {message}");
    ExitCode::from(2)
}

fn finish(result: Result<Report, String>) -> ExitCode {
    match result {
        Ok(report) if report.lines.is_empty() => {
            println!("dependencies are up to date");
            ExitCode::SUCCESS
        }
        Ok(report) => {
            for line in &report.lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => failure(&e),
    }
}

impl Command for  CommandVersion {
    fn execute(&self) -> ExitCode {
        println!("{}", self.version);
        ExitCode::SUCCESS
    }
}

impl Command for CommandUsage {
    fn execute(&self) -> ExitCode {
        if self.error {
            eprint!("{HELP}");
            ExitCode::from(2)
        }
        else {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
    }
}

impl CommandNew {
    fn new(args: Args) -> Self {
        Self { args: args.collect() }
    }

    fn create_dir(&self) -> std::io::Result<()> {
        let cwd = std::env::current_dir()?;
        let dir_name = self.args.last().unwrap();
        let dir_path = cwd.join(dir_name);
        std::fs::create_dir_all(dir_path)?;
        Ok(())
    }

    fn create_dir_internal(&self, dir_name: String) -> std::io::Result<()> {
        let cwd = std::env::current_dir()?;
        let root_dir = self.args.last().unwrap();
        let dir_path = cwd.join(root_dir).join(dir_name);
        std::fs::create_dir_all(dir_path)?;
        Ok(())
    }

    fn create_file(&self, filepath: String, filecontent: &[u8]) -> std::io::Result<()> {
        let cwd = std::env::current_dir()?;
        let root_dir = self.args.last().unwrap();
        let path = cwd.join(root_dir).join(filepath);
        let mut file = std::fs::File::create(path)?;
        file.write_all(filecontent)?;
        
        return Ok(())
    }
}

impl CommandUsage {
    fn new(error: bool) -> Self {
        Self{ error }
    }
}

impl  CommandVersion {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

impl Cli {
    pub fn new(args: Args) -> Self {
        Self { args: args }
    }

    pub fn command(self) -> Box<dyn Command> {
        let mut args = self.args;
        args.next();
        let command = args.next();
        let help = std::env::args().skip(2).any(|arg| arg == "--help" || arg == "-h");
        let cmd: Box<dyn Command> = match command.as_deref() {
            Some(_) if help                       => Box::new(CommandUsage::new(false)),
            Some("new")                           => Box::new(CommandNew::new(args)),
            Some("install")                       => Box::new(CommandInstall { args: args.collect() }),
            Some("remove")                        => Box::new(CommandRemove { args: args.collect() }),
            Some("update")                        => Box::new(CommandUpdate { args: args.collect() }),
            Some("--help" | "-h" | "help") | None => Box::new(CommandUsage::new(false)),
            Some("--version" | "-v" | "version")  => Box::new(CommandVersion::new(String::from(VERSION))),
            Some(_)                               => Box::new(CommandUsage::new(true)),
        };
        cmd
    }

    //pub fn what(&self) {
    //    println!("hehehe");
    //}
    pub fn execute(&self) -> ExitCode {
        ExitCode::from(2)
    }
}
