//! `sparrow`: the Sparrow runtime on the core (`sparrow-core`): loading with a
//! size limit, execution, tests, verification, assembler and disassembler.
use std::io::{Read, Write};
use std::{fs::File, path::Path, process::ExitCode};

#[global_allocator]
#[cfg(not(feature = "allocation-stats"))]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;


#[cfg(feature = "allocation-stats")]
#[global_allocator]
static ALLOCATOR: okkama_allocation_counter::Counting = okkama_allocation_counter::Counting;

const HELP: &str = "Sparrow runtime

USAGE:
    sparrow [--stats] <FILE.spb> [ARGS...]
    sparrow run [--stats] [BUDGETS] <FILE.spb> [ARGS...]
    sparrow test <FILE.spb>
    sparrow check <FILE.spb>
    sparrow --assemble <FILE.spa> -o <FILE.spb>
    sparrow --disassemble <FILE.spb> [-o <FILE.spa>]
    sparrow --native-capabilities
    sparrow --version

run          Verify and execute bytecode; ARGS are the program's arguments.
             main's integer result in 0..255 becomes the exit status; an
             uncaught failure is reported on stderr (exit 1) with the call
             stack in Sparrow terms (OKKAMA_TRACE=off: without). `--stats` prints
             counts of runtime events at exit (also SPARROW_CORE_STATS=1),
             with the compile time and machine-code size of each function.
             DEBUGGING the compiled code (SYMBOLS: comma-separated, or *):
               SPARROW_CORE_DUMP=SYMBOLS         CLIF and machine code to stderr
               SPARROW_CORE_UNOPTIMIZED=SYMBOLS  compile without Cranelift's passes
               SPARROW_CORE_PLAIN=SYMBOLS        compile without inline fast paths
             BUDGETS (all off by default; running out exits 1 with
             \"Execution budget exhausted: …\" or \"Execution interrupted\"):
               --max-steps N      stop after N steps (function entries, loop iterations)
               --max-memory SIZE  stop when the run holds more than SIZE bytes (K/M/G)
               --max-output SIZE  stop when stdout and stderr exceed SIZE bytes
               --timeout SECONDS  interrupt the run after SECONDS of wall-clock time
test         Run the tests of bytecode built with `okkama build -t`
             (`sparrow --test FILE.spb` is the same).
check        Verify bytecode (types, calls, intrinsic contracts) without
             executing it.
--assemble   Assemble Sparrow assembly text into bytecode.
--disassemble  Print bytecode as Sparrow assembly (stdout without -o).
--native-capabilities  List natives (JSON) and whether this runtime has them.

SPB 9 is supported. Source files, the Okkama compiler and Java are not
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

fn load(path: &Path) -> Result<(Vec<u8>, okkama_sparrow::Program), Failure> {
    let bytes = read(path)?;
    let program = okkama_sparrow::decode(&bytes).map_err(|e| (1, e.to_string()))?;
    Ok((bytes, program))
}

fn write(path: &str, bytes: &[u8]) -> Result<u8, Failure> {
    std::fs::write(path, bytes).map_err(|e| (3, format!("cannot write `{path}`: {e}")))?;
    Ok(0)
}

/// Process workers run the same file in `sparrow --worker FILE.spb`.
fn worker_command(path: &Path) -> Vec<std::ffi::OsString> {
    let exe = std::env::current_exe().unwrap_or_else(|_| "sparrow".into());
    let file = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    vec![exe.into_os_string(), file.into_os_string()]
}

/// An uncaught failure is the program's report; it ends with a newline.
fn report(error: sparrow_core::Error) -> Result<u8, Failure> {
    let text = error.0;
    eprint!("{text}{}", if text.ends_with('\n') { "" } else { "\n" });
    Ok(1)
}

/// Stop the process with the interrupt report one second after `flag` is
/// raised, if the run has not stopped by then: a native that cannot look at
/// the flag (Argon2, a blocking read) still ends (#238).
fn watchdog(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let _ = std::thread::Builder::new().name("sparrow-watchdog".into()).spawn(move || {
        while !flag.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        sparrow_core::stdout::try_flush();
        let _ = std::io::Write::flush(&mut std::io::stdout());
        eprintln!("{}", sparrow_core::budget::Exhausted::Interrupted.message());
        std::process::exit(1);
    });
}

fn execute(path: &Path, arguments: &[String], stats: bool, limits: sparrow_core::budget::Limits) -> Result<u8, Failure> {
    let (bytes, program) = load(path)?;
    if let Some(flag) = &limits.interrupt {
        watchdog(flag.clone());
    }
    let arguments = arguments.iter().map(|a| a.clone().into_bytes()).collect();
    let result = sparrow_core::run_described(bytes, arguments, stats, worker_command(path), limits);
    let _ = sparrow_core::stdout::flush();
    match result {
        Ok(Ok(code)) => u8::try_from(code).map_err(|_| (1, format!("main() must return an int in 0..255, got {code}"))),
        Ok(Err((error, uncaught))) => {
            let code = report(error);
            // `OKKAMA_TRACE=off` — the failure alone, as with `okkama run`.
            if let Some(uncaught) = uncaught.filter(|_| std::env::var("OKKAMA_TRACE").as_deref() != Ok("off")) {
                eprint!("{}", sparrow_core::render_trace(&program, &uncaught.trace));
            }
            code
        }
        Err(error) => report(error),
    }
}

fn test(path: &Path) -> Result<u8, Failure> {
    let (bytes, _) = load(path)?;
    match sparrow_core::run_tests(bytes, Box::new(sparrow_core::stdout::Buffered)) {
        Ok(passed) => Ok(u8::from(!passed)),
        Err(error) => Err((1, error.0)),
    }
}

/// The child side of a process worker: the request on stdin, the response
/// on stdout; it never runs main.
fn answer(path: &Path) -> Result<u8, Failure> {
    let (bytes, _) = load(path)?;
    let mut request = Vec::new();
    std::io::stdin().read_to_end(&mut request).map_err(|e| (3, format!("worker request: {e}")))?;
    let response = sparrow_core::run_worker(bytes, request, worker_command(path)).map_err(|e| (1, e.0))?;
    // The action's own output, buffered (#132), comes before the response.
    sparrow_core::stdout::flush().map_err(|e| (3, format!("worker response: {e}")))?;
    let mut out = std::io::stdout().lock();
    out.write_all(&response).and_then(|()| out.flush()).map_err(|e| (3, format!("worker response: {e}")))?;
    Ok(0)
}

fn capabilities() -> u8 {
    let entries: Vec<_> = sparrow_core::native_capabilities().collect();
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "[");
    for (index, (symbol, supported)) in entries.iter().enumerate() {
        let comma = if index + 1 == entries.len() { "" } else { "," };
        let _ = writeln!(out, "  {{\"symbol\":\"{symbol}\",\"supported\":{supported}}}{comma}");
    }
    let _ = writeln!(out, "]");
    0
}

fn run() -> Result<u8, Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = || (2, HELP.to_string());
    let worker = args.first().is_some_and(|a| a == "--worker");
    if std::env::var_os("SPARROW_INTERNAL_WORKER").is_some() && !worker {
        return Err((2, "worker launch rejected: --worker must precede FILE.spb; refusing to run main".into()));
    }
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
        "--native-capabilities" if rest.is_empty() => Ok(capabilities()),
        "--worker" => match rest {
            [path] => answer(Path::new(path)),
            _ => Err(usage()),
        },
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
            let text = okkama_sparrow::disassemble(&load(Path::new(input))?.1);
            match output {
                Some(output) => write(output, text.as_bytes()),
                None => {
                    print!("{text}");
                    Ok(0)
                }
            }
        }
        "--test" => match rest {
            [path] => test(Path::new(path)),
            _ => Err(usage()),
        },
        "run" | "check" | "test" => {
            let mut stats = false;
            let mut limits = sparrow_core::budget::Limits::default();
            let mut rest = rest;
            // Options belong to `run` only.
            while let ("run", [flag, tail @ ..]) = (first.as_str(), rest) {
                match (flag.as_str(), tail) {
                    ("--stats", _) => {
                        stats = true;
                        rest = tail;
                    }
                    (_, [value, tail @ ..]) if limits.option(flag, value).map_err(|e| (2, e))? => rest = tail,
                    _ => break,
                }
            }
            let Some((path, arguments)) = rest.split_first() else {
                return Err(usage());
            };
            if path == "--help" || path == "-h" {
                print!("{HELP}");
                return Ok(0);
            }
            match first.as_str() {
                "check" if arguments.is_empty() => {
                    let bytes = read(Path::new(path))?;
                    sparrow_core::check(&bytes).map(|()| 0).map_err(|e| (1, e.0))
                }
                "test" if arguments.is_empty() => test(Path::new(path)),
                "run" => execute(Path::new(path), arguments, stats, limits),
                _ => Err(usage()),
            }
        }
        "--stats" => match rest.split_first() {
            Some((path, arguments)) => execute(Path::new(path), arguments, true, Default::default()),
            None => Err(usage()),
        },
        // `--cold` was a switch of the previous VM; the core always compiles.
        "--cold" => match rest.split_first() {
            Some((path, arguments)) => execute(Path::new(path), arguments, false, Default::default()),
            None => Err(usage()),
        },
        option if option.starts_with('-') => Err((2, format!("unknown option `{option}`\n{HELP}"))),
        path => {
            if !Path::new(path).exists() && !path.ends_with(".spb") {
                return Err((2, format!("unknown command `{path}`\n{HELP}")));
            }
            execute(Path::new(path), rest, false, Default::default())
        }
    }
}

fn main() -> ExitCode {
    #[cfg(feature = "allocation-stats")]
    let _allocation_report = okkama_allocation_counter::Report;
    #[cfg(not(feature = "allocation-stats"))]
    sparrow_core::budget::set_memory_probe(sparrow_core::budget::mimalloc_committed);
    let result = run();
    // The program's buffered output goes before a failure report (#132).
    let _ = sparrow_core::stdout::flush();
    match result {
        Ok(code) => ExitCode::from(code),
        Err((code, message)) => {
            eprintln!("sparrow: {message}");
            ExitCode::from(code)
        }
    }
}
