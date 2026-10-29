//! Core library for the Blade command-line tool.

use std::{env::Args, io::Write, process::ExitCode};
//use std::path::{Path, PathBuf};

/// Help text shared by Blade command-line front ends.
pub const HELP: &str = "\
blade — Blade command-line tool

USAGE:
    blade --help
    blade --version
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

        if let Err(e) = self.create_dir_internal(String::from("lib")) {
            eprintln!("cannot make lib dir: {}", e);
            return ExitCode::from(2);
        }

        let project_name = self.args.last().unwrap();
        
        // create src/main.ok
        /*
            module hello v1.0.0;

            import "io/stdout" take Stdout;

            main() is int {
                Stdout.writeln("hello from okkama");
                0;
            }
        */
        let main_ok = format!(
    r#"module {project_name} v1.0.0;
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

        

        // create hello.toml
        /*
            [module]
            name = "hello"
            version = 1.0.0
            
            [build]
            target = "sparrow"
            output = "hello"
        */
        let hello_toml = format!(
r#"[module]
name = "{project_name}"
version = "1.0.0"

[build]
target = "sparrow"
output = "{project_name}"
"#
);
        if let Err(e) = self.create_file(String::from(format!("{project_name}.toml")), hello_toml.as_bytes()) {
            eprintln!("cannot make file hello.toml: {}", e);
            return ExitCode::from(2);
        }

        ExitCode::SUCCESS
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
        let cmd: Box<dyn Command> = match command.as_deref() {
            Some("new")                           => Box::new(CommandNew::new(args)),
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
