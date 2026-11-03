use sparrow_engine::{Executable, Mode};

struct Options {
    mode: Mode,
    path: std::ffi::OsString,
    arguments: Vec<std::ffi::OsString>,
    worker: bool,
    tests: bool,
}

fn parse_arguments(
    args: impl IntoIterator<Item = std::ffi::OsString>,
    worker_expected: bool,
) -> Result<Options, &'static str> {
    let mut mode = Mode::Jit;
    let mut path = None;
    let mut arguments = Vec::new();
    let mut worker = false;
    let mut tests = false;
    for arg in args {
        if path.is_some() {
            arguments.push(arg);
        } else if arg == "--test" {
            tests = true;
        } else if arg == "--worker" {
            worker = true;
        } else if arg == "--cold" {
            mode = Mode::Cold;
        } else {
            path = Some(arg);
        }
    }
    if worker_expected && !worker {
        return Err("worker launch rejected: --worker must precede FILE.spb; refusing to run main");
    }
    let path = path.ok_or("usage: sparrow-next [--cold] [--test] [--worker] FILE.spb")?;
    Ok(Options {
        mode,
        path,
        arguments,
        worker,
        tests,
    })
}

fn main() -> std::process::ExitCode {
    if std::env::args_os().len() == 2
        && std::env::args_os()
            .nth(1)
            .is_some_and(|a| a == "--native-capabilities")
    {
        println!("[");
        let entries: Vec<_> = sparrow_engine::native_capabilities().collect();
        for (index, (symbol, supported)) in entries.iter().enumerate() {
            println!(
                "  {{\"symbol\":\"{symbol}\",\"supported\":{supported}}}{}",
                if index + 1 == entries.len() { "" } else { "," }
            );
        }
        println!("]");
        return 0.into();
    }

    let Options {
        mode,
        path,
        arguments,
        worker,
        tests,
    } = match parse_arguments(
        std::env::args_os().skip(1),
        std::env::var_os("SPARROW_INTERNAL_WORKER").is_some(),
    ) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("sparrow-next: {error}");
            return 2.into();
        }
    };
    let mut worker_command = vec![
        std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("sparrow-next"))
            .into_os_string(),
    ];
    if matches!(mode, Mode::Cold) {
        worker_command.push("--cold".into());
    }
    worker_command.push(
        std::fs::canonicalize(&path)
            .unwrap_or_else(|_| std::path::PathBuf::from(&path))
            .into_os_string(),
    );
    if worker {
        use std::io::{Read, Write};
        let result = (|| -> Result<(), String> {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let program = okkama_sparrow::decode(&bytes).map_err(|e| e.to_string())?;
            let mut request = Vec::new();
            std::io::stdin()
                .read_to_end(&mut request)
                .map_err(|e| e.to_string())?;
            let response =
                Executable::worker_response_with_command(&program, &request, mode, worker_command)
                    .map_err(|e| e.to_string())?;
            std::io::stdout()
                .write_all(&response)
                .and_then(|()| std::io::stdout().flush())
                .map_err(|e| e.to_string())
        })();
        return match result {
            Ok(()) => 0.into(),
            Err(error) => {
                eprintln!("sparrow-next worker: {error}");
                1.into()
            }
        };
    }
    if tests {
        let result = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| okkama_sparrow::decode(&bytes).map_err(|e| e.to_string()))
            .and_then(|program| {
                sparrow_engine::run_tests_with_worker_command(
                    &program,
                    mode,
                    worker_command,
                    &mut std::io::stdout(),
                )
                .map_err(|e| e.to_string())
            });
        return match result {
            Ok(true) => 0.into(),
            Ok(false) => 1.into(),
            Err(error) => {
                eprintln!("sparrow-next: {error}");
                1.into()
            }
        };
    }
    let result = std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| Executable::decode(&bytes, mode).map_err(|e| e.to_string()))
        .and_then(|program| {
            program
                .with_arguments(arguments)
                .and_then(|program| program.with_worker_command(worker_command))
                .map_err(|e| e.to_string())
        })
        .and_then(|program| {
            program
                .run()
                .map_err(|e| e.to_string())?
                .to_i64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or(())
                .map_err(|_| "entry result outside 0..255".to_owned())
        });
    match result {
        Ok(code) => code.into(),
        Err(error) => {
            eprintln!("sparrow-next: {error}");
            1.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str], worker_expected: bool) -> Result<Options, &'static str> {
        parse_arguments(args.iter().map(std::ffi::OsString::from), worker_expected)
    }

    #[test]
    fn worker_launch_tests_accept_flag_before_file() {
        for args in [
            vec!["--worker", "program.spb"],
            vec!["--worker", "--cold", "program.spb"],
            vec!["--worker", "--test", "program.spb"],
        ] {
            let options = parse(&args, true).unwrap();
            assert!(options.worker);
            assert_eq!(options.path, "program.spb");
            assert!(options.arguments.is_empty());
        }
    }

    #[test]
    fn worker_launch_tests_reject_old_recursive_command_before_execution() {
        for args in [
            vec!["program.spb", "--worker"],
            vec!["--cold", "program.spb", "--worker"],
            vec!["program.spb"],
        ] {
            assert!(
                matches!(parse(&args, true), Err(message) if message.contains("refusing to run main"))
            );
        }
    }

    #[test]
    fn worker_launch_tests_preserve_language_arguments_after_file() {
        let options = parse(&["--cold", "program.spb", "--worker", "--test"], false).unwrap();
        assert!(!options.worker);
        assert!(!options.tests);
        assert!(matches!(options.mode, Mode::Cold));
        assert_eq!(
            options.arguments,
            vec![std::ffi::OsString::from("--worker"), "--test".into()]
        );
    }
}
