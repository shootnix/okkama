//! `sparrow-next`: the Sparrow runtime CLI on the rebuilt core.
//!
//!     sparrow-next [--test] FILE.spb [ARGUMENTS…]
//!     sparrow-next --native-capabilities
//!
//! Arguments after FILE belong to the program. `--cold` is accepted for
//! compatibility (the core always compiles). Process workers (`--worker`) are
//! not supported by this core yet.
use std::io::Write;

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
    let mut path = None;
    let mut arguments = Vec::new();
    for arg in args {
        if path.is_some() {
            arguments.push(arg.into_encoded_bytes());
        } else if arg == "--test" {
            tests = true;
        } else if arg == "--cold" {
        } else if arg == "--worker" || arg == "--stats" {
            eprintln!("sparrow-next: {} is not supported by sparrow-core yet", arg.to_string_lossy());
            return 2.into();
        } else {
            path = Some(arg);
        }
    }
    let Some(path) = path else {
        eprintln!("sparrow-next: usage: sparrow-next [--test] FILE.spb [ARGUMENTS...]");
        return 2.into();
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("sparrow-next: {error}");
            return 1.into();
        }
    };
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
    match sparrow_core::run_with(bytes, arguments) {
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
