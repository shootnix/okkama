//! Core library for the Blade command-line tool.

use std::{env::Args, process::ExitCode};
use std::path::{Path, PathBuf};

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
        let result = self.create_dir();
        match result {
            Ok(()) => {},
            Err(e) => {
                eprintln!("{}", e);
            }
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
