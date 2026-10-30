//! Автономная VM Sparrow: загрузка файла с пределом размера, исполнение,
//! тесты, проверка, ассемблер и дизассемблер; коды завершения процесса.
use std::{fs::File, io::Read, path::Path, process::ExitCode};

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

const HELP: &str = "Sparrow virtual machine

USAGE:
    sparrow <FILE.spb> [ARGS...]
    sparrow run <FILE.spb> [ARGS...]   (compatibility alias)
    sparrow test <FILE.spb>
    sparrow check <FILE.spb>
    sparrow --assemble <FILE.spa> -o <FILE.spb>
    sparrow --disassemble <FILE.spb> [-o <FILE.spa>]
    sparrow --version

run          Verify and execute bytecode in this process; ARGS are the
             program's arguments. main's integer result in 0..255 becomes the
             exit status; an uncaught failure is reported on stderr (exit 1)
             with the call stack in Sparrow terms (OKKAMA_TRACE=off: without).
test         Run the tests of bytecode built with `okkama build -t`.
check        Verify bytecode without executing it (including infinite loops).
--assemble   Assemble Sparrow assembly text into bytecode.
--disassemble  Print bytecode as Sparrow assembly (stdout without -o).

SPB 9 is supported. Source files, Okkama compiler and Java are not
required. Format/runtime errors exit 1, usage errors 2, I/O errors 3.
";

type Failure = (u8, String);

fn read(path: &Path) -> Result<Vec<u8>, Failure> {
    let file = File::open(path).map_err(|e| (3, format!("cannot read `{}`: {e}", path.display())))?;
    let mut bytes = Vec::new();
    file.take(okkama_sparrow::MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| (3, format!("cannot read `{}`: {e}", path.display())))?;
    Ok(bytes)
}

fn load(path: &Path) -> Result<okkama_sparrow::Program, Failure> {
    okkama_sparrow::decode(&read(path)?).map_err(|e| (1, e.to_string()))
}

fn write(path: &str, bytes: &[u8]) -> Result<u8, Failure> {
    std::fs::write(path, bytes).map_err(|e| (3, format!("cannot write `{path}`: {e}")))?;
    Ok(0)
}

/// Воркеры `spawn` — тот же файл в новом процессе этой VM.
fn worker(path: &Path) {
    let exe = std::env::current_exe().map_or_else(|_| "sparrow".to_string(), |exe| exe.display().to_string());
    let file = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    sparrow_vm::set_worker_command(vec![exe, "run".into(), file.display().to_string()]);
}

fn execute(path: &Path, arguments: Vec<String>) -> Result<u8, Failure> {
    let program = load(path)?;
    worker(path);
    let (result, trace) = sparrow_vm::execute_traced(&program, arguments);
    match result {
        Ok(value) => value
            .to_string()
            .parse::<u8>()
            .map_err(|_| (1, format!("main() must return an int in 0..255, got {value}"))),
        Err(error) => match sparrow_vm::report(&error) {
            Some(text) => {
                eprint!("{text}");
                // `OKKAMA_TRACE=off` — без стека (как у `okkama run`).
                if std::env::var("OKKAMA_TRACE").as_deref() != Ok("off") {
                    eprint!("{}", sparrow_vm::render_trace(&program, &trace));
                }
                Ok(1)
            }
            None => Err((1, error.to_string())),
        },
    }
}

fn run() -> Result<u8, Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = || (2, HELP.to_string());
    let Some((first, rest)) = args.split_first() else {
        return Err(usage());
    };
    match first.as_str() {
        "--help" | "-h" | "help" => {
            print!("{HELP}");
            Ok(0)
        }
        "--version" | "-V" => {
            println!("sparrow {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        "--assemble" => {
            let [input, flag, output] = rest else {
                return Err(usage());
            };
            if flag != "-o" {
                return Err(usage());
            }
            let text = String::from_utf8(read(Path::new(input))?).map_err(|_| (1, format!("`{input}` is not UTF-8 text")))?;
            let program = okkama_sparrow::assemble(&text).map_err(|e| (1, e.to_string()))?;
            let bytes = okkama_sparrow::encode(&program).map_err(|e| (1, e.to_string()))?;
            write(output, &bytes)
        }
        "--disassemble" => {
            let (input, output) = match rest {
                [input] => (input, None),
                [input, flag, output] if flag == "-o" => (input, Some(output)),
                _ => return Err(usage()),
            };
            let text = okkama_sparrow::disassemble(&load(Path::new(input))?);
            match output {
                Some(output) => write(output, text.as_bytes()),
                None => {
                    print!("{text}");
                    Ok(0)
                }
            }
        }
        "run" | "check" | "test" => {
            let Some((path, arguments)) = rest.split_first() else {
                return Err(usage());
            };
            if path == "--help" || path == "-h" {
                print!("{HELP}");
                return Ok(0);
            }
            match first.as_str() {
                "check" if arguments.is_empty() => {
                    load(Path::new(path))?;
                    Ok(0)
                }
                "test" if arguments.is_empty() => {
                    let program = load(Path::new(path))?;
                    worker(Path::new(path));
                    let mut stdout = std::io::stdout();
                    match sparrow_vm::run_tests(&program, &mut stdout) {
                        Ok(passed) => Ok(u8::from(!passed)),
                        Err(error) => Err((1, error.to_string())),
                    }
                }
                "run" => execute(Path::new(path), arguments.to_vec()),
                _ => Err(usage()),
            }
        }
        option if option.starts_with('-') => Err((2, format!("unknown option `{option}`\n{HELP}"))),
        path => {
            if !Path::new(path).exists() && !path.ends_with(".spb") {
                return Err((2, format!("unknown command `{path}`\n{HELP}")));
            }
            execute(Path::new(path), rest.to_vec())
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err((code, message)) => {
            eprintln!("sparrow: {message}");
            ExitCode::from(code)
        }
    }
}
