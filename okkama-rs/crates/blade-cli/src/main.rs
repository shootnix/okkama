use std::process::ExitCode;

use blade::Cli;
use blade::{HELP, VERSION};

fn main() -> ExitCode {
    let cli = Cli::new(std::env::args());
    let cmd = cli.command();
    cmd.execute()
    //cli.execute()
    /*
    match std::env::args().nth(1).as_deref() {
        Some("--help" | "-h" | "help") | None => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("blade {VERSION}");
            ExitCode::SUCCESS
        }
        Some(_) => {
            //cli.what();
            eprint!("{HELP}");
            ExitCode::from(2)
        }
    }
    */
}
