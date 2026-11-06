//! `sparrow-next`: the Sparrow runtime CLI on the rebuilt core.
//!
//!     sparrow-next [--test] [--stats] FILE.spb [ARGUMENTS…]
//!     sparrow-next --native-capabilities
//!
//! Arguments after FILE belong to the program. `--cold` is accepted for
//! compatibility (the core always compiles). `--stats` prints counts of runtime
//! events to stderr at exit. `--worker FILE.spb` is the child side of a
//! process worker: it answers the request on stdin and never runs main (a
//! child marked by `SPARROW_INTERNAL_WORKER` refuses to start without it).
use std::io::{Read, Write};

fn main() -> std::process::ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--native-capabilities" {
        let entries: Vec<_> = sparrow_core::native_capabilities().collect();
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "[");
        for (index, (symbol, supported)) in entries.iter().enumerate() {
            let comma = if index + 1 == entries.len() { "" } else { "," };
            let _ = writeln!(out, "  {{\"symbol\":\"{symbol}\",\"supported\":{supported}}}{comma}");
        }
        let _ = writeln!(out, "]");
        return 0.into();
    }
    let mut tests = false;
    let mut stats = false;
    let mut worker = false;
    let mut path = None;
    let mut arguments = Vec::new();
    for arg in args {
        if path.is_some() {
            arguments.push(arg.into_encoded_bytes());
        } else if arg == "--test" {
            tests = true;
        } else if arg == "--cold" {
        } else if arg == "--stats" {
            stats = true;
        } else if arg == "--worker" {
            worker = true;
        } else {
            path = Some(arg);
        }
    }
    if std::env::var_os("SPARROW_INTERNAL_WORKER").is_some() && !worker {
        eprintln!("sparrow-next: worker launch rejected: --worker must precede FILE.spb; refusing to run main");
        return 2.into();
    }
    let Some(path) = path else {
        eprintln!("sparrow-next: usage: sparrow-next [--test] [--stats] [--worker] FILE.spb [ARGUMENTS...]");
        return 2.into();
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("sparrow-next: {error}");
            return 1.into();
        }
    };
    let worker_command = vec![
        std::env::current_exe().unwrap_or_else(|_| "sparrow-next".into()).into_os_string(),
        std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone().into()).into_os_string(),
    ];
    if worker {
        let mut request = Vec::new();
        let answered = std::io::stdin()
            .read_to_end(&mut request)
            .map_err(|e| e.to_string())
            .and_then(|_| sparrow_core::run_worker(bytes, request, worker_command).map_err(|e| e.0))
            .and_then(|response| {
                let mut out = std::io::stdout().lock();
                out.write_all(&response).and_then(|()| out.flush()).map_err(|e| e.to_string())
            });
        return match answered {
            Ok(()) => 0.into(),
            Err(error) => {
                eprintln!("sparrow-next worker: {error}");
                1.into()
            }
        };
    }
    if tests {
        return match sparrow_core::run_tests(bytes, Box::new(std::io::stdout())) {
            Ok(true) => 0.into(),
            Ok(false) => 1.into(),
            Err(error) => {
                eprintln!("sparrow-next: {error}");
                1.into()
            }
        };
    }
    match sparrow_core::run_configured(bytes, arguments, stats, worker_command) {
        Ok(code) => u8::try_from(code).map_or_else(
            |_| {
                eprintln!("sparrow-next: entry result outside 0..255");
                std::process::ExitCode::from(1)
            },
            std::process::ExitCode::from,
        ),
        Err(error) => {
            eprintln!("sparrow-next: {error}");
            1.into()
        }
    }
}
