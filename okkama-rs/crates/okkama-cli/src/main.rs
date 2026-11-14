//! `okkama` — command-line front end for the Okkama compiler library.
//!
//! The binary owns everything the library deliberately does not do: reading
//! files and stdin, parsing command-line options, choosing output streams
//! and exit codes. Results go to stdout, diagnostics to stderr.
//!
//! Exit codes: 0 success, 1 a source/lowering/codegen error, 2 invalid command
//! line, 3 an input/output error.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[global_allocator]
#[cfg(not(feature = "allocation-stats"))]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;


#[cfg(feature = "allocation-stats")]
use okkama_allocation_counter as allocation_stats;
#[cfg(feature = "allocation-stats")]
#[global_allocator]
static ALLOCATOR: allocation_stats::Counting = allocation_stats::Counting;

mod core;
mod import_paths;
mod lsp;
mod man;
mod ocs;
mod trace;

use blade::manifest;
use okkama_compiler::codegen::{self, Backend, Options as CodegenOptions};
use okkama_compiler::diagnostics::{Diagnostic, render_all};
use okkama_compiler::ir;
use okkama_compiler::lowering::{
    self, LinkError, LoadedModule, LoweringError, ModuleLoader, RootFile,
};
use okkama_compiler::source::{SourceFile, Span};
use okkama_compiler::syntax::print::{
    Format, PrintOptions, print_program, tokens_json, tokens_text,
};
use okkama_compiler::syntax::{self, ParseMode, ParseOptions, ast, lexer, prune};

const EXIT_DIAGNOSTICS: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_IO: u8 = 3;

const HELP: &str = "\
okkama — Okkama language front end and IR

USAGE:
    okkama tokens <FILE|-> [--format text|json]
    okkama ast <FILE|-> [--format tree|json] [--spans] [--mode full|production] [--prune-tests]
    okkama check <FILE|->
    okkama fmt [-w|--check] <FILE|->...
    okkama lint <FILE>...
    okkama man [MODULE [NAME | NAME.ACTION]] [--format text|markdown]
    okkama lsp
    okkama env
    okkama ir <FILE|-> [--format text]
    okkama build <FILE|-> --target jvm [--budgeted] -o <FILE>
    okkama build <FILE|-> --target sparrow -o <FILE.spb>
    okkama exec [BUDGETS] <FILE.spb>
    okkama run [-t [--filter TEXT]...] <FILE> [FILE.ok...] [ARGS...]
    okkama run --engine sparrow|jvm <FILE> [FILE.ok...]
    okkama <FILE.ok> [FILE.ok...]
    okkama help [COMMAND]
    okkama --version

COMMANDS:
    tokens    Print the token stream of a source file
    ast       Parse a source file and print its syntax tree
    check     Check a program and its imports; report every compile-time error
    fmt       Format source files (`-w` rewrites them, `--check` only reports)
    lint      Check programs and report errors, warnings and lint findings
    man       Show the reference page of a module, a declaration or an action,
              from its doc comments; without MODULE, the index of std
    env       Show where okkama finds std, the native library, ICU and Java
    lsp       Run the language server (LSP over stdio) for editors
    ir        Lower a source module to IR and print its functions
    build     Lower a source module and invoke the selected backend
    exec      Execute a Sparrow bytecode file in this process
    run       Run a program on Sparrow (JIT); `--engine jvm` builds and runs Java

Use `-` as FILE to read the source from standard input. `check`, `ir` and
`build` accept several FILE paths: the files of one module (spec §5.1) share
declarations, each keeps its own imports. With `--tests` (`-t`) they work in
test mode (spec §14): `build` produces a test runner instead of `main`.

AST OPTIONS:
    --format tree|json     Output format (default: tree)
    --spans                Include source spans (byte offsets, lines, columns)
    --mode full            Parse test and fake bodies (default)
    --mode production      Skip test/fake bodies as balanced braces (spec §14);
                           they appear as SkippedBody nodes
    --prune-tests          Remove test and fake declarations after parsing

TOKENS OPTIONS:
    --format text|json     Output format (default: text)

IR OPTIONS:
    --format text          Output format (default: text); see `okkama help ir`

BUILD OPTIONS:
    --target jvm          Select the backend (required; the JAR runs on JDK 22+)
    --target sparrow      Select Sparrow bytecode (.spb)
    --backend NAME        Compatibility alias for --target
    -o, --output FILE      Output artifact path (required)
    --budgeted            JVM: count steps (`java -Dokkama.steps=N`)

RUN OPTIONS:
    --engine sparrow      Compile to Sparrow and execute it with JIT (default)
    --engine jvm          Compile to JVM classes and execute them with `java`
                          Place engine options before FILE; every engine runs
                          tests (-t), program arguments and `spawn` workers.
                          OKKAMA_ENGINE=sparrow|jvm sets the default.
    --filter TEXT         With -t: run only the tests whose name contains
                          TEXT (literally, case-sensitive); repeated, the
                          tests that contain any of the texts
    --max-steps N         Stop after N steps: function entries and loop
                          iterations (off by default), counted alike on
                          every engine; on the jvm only a run with this
                          option generates the counting code
    --max-memory SIZE     Stop when the run holds more than SIZE bytes
                          (K/M/G suffixes); also bounds recursion depth. On
                          the jvm: the Java heap (-Xmx, runtime included)
                          and every thread stack
    --max-output SIZE     Stop when stdout and stderr exceed SIZE bytes
    --timeout SECONDS     Interrupt the run after SECONDS of wall-clock time
                          Running out exits 1: \"Execution budget exhausted:
                          steps|memory|output\" or \"Execution interrupted\".
                          An uncaught failure on Sparrow also
                          prints its source line and the call stack;
                          OKKAMA_TRACE=off prints the failure alone.

EXIT STATUS:
    0  success
    1  a source, lowering or codegen error (including invalid UTF-8)
    2  invalid command line
    3  an input/output error
";

const TOKENS_HELP: &str = "\
okkama tokens — print the token stream of a source file

USAGE:
    okkama tokens <FILE|-> [--format text|json]

Text output prints one token per line: the position range LINE:COL-LINE:COL
(end exclusive, columns count Unicode characters), the token kind, the
quoted source spelling and, for literals, the decoded value. On lexical
errors nothing is printed to stdout, diagnostics go to stderr and the exit
status is 1.
";

const AST_HELP: &str = "\
okkama ast — parse a source file and print its syntax tree

USAGE:
    okkama ast <FILE|-> [--format tree|json] [--spans] [--mode full|production] [--prune-tests]

OPTIONS:
    --format tree|json     Output format (default: tree)
    --spans                Include source spans
    --mode full            Parse test and fake bodies (default)
    --mode production      Consume test/fake bodies as balanced braces without
                           parsing them (SkippedBody nodes); lexical errors
                           inside them still count
    --prune-tests          Remove test and fake declarations from the tree

The whole input must parse. On any error nothing is printed to stdout,
diagnostics go to stderr and the exit status is 1. Imports are printed as
syntax; the imported modules are not loaded.
";

const IR_HELP: &str = "\
okkama ir — print module and function IR

USAGE:
    okkama ir <FILE|-> [--format text] [--passes LIST | --after PASS]

OPTIONS:
    --format text     Output format (default: text)
    --passes LIST     Run only these IR passes: none, all, or a comma list of
                      regions, tail-calls, fold, hoist, undef, jumps (in that order)
    --after PASS      The IR after PASS: the passes up to it in that order

Input is a complete Okkama program: one or several source files of one module
(spec §5.1). Imports are loaded (import directories, the importing file's
directory, std) and linked into one OIR-1 module: declarations, functions with
blocks ending in `ret`, `br`, `cond`, … Analysis warnings go to stderr.
Constructs the IR does not support yet produce a lowering error; no source
declarations or statements are silently discarded.

Use `-` as FILE to read standard input. This command prints IR without
executing the program.
On any error stdout is empty; diagnostics go to stderr. Source and lowering
errors exit with code 1, usage errors with code 2, and input read errors with 3.
";

const BUILD_HELP: &str = "\
okkama build — invoke a code generation backend

USAGE:
    okkama build <FILE|-> --target jvm [--budgeted] -o <FILE>
    okkama build <FILE|-> --target sparrow -o <FILE.spb>

OPTIONS:
    --target jvm          Select the backend (required)
    --target sparrow      Generate Sparrow bytecode
    --backend NAME        Compatibility alias for --target
    -o, --output FILE      Output artifact path (required)
    --budgeted            JVM: count steps; the limit comes from
                          `java -Dokkama.steps=N` (without it, no limit)

Use `-` as input to read Okkama source from standard input.
The pipeline is source -> AST -> module IR -> selected backend -> output file.

The JVM backend names the main class after the output file stem. `-o out.jar`
writes one executable JAR (program classes, the okkama.rt runtime, manifest),
runnable with `java -jar out.jar`. `-o out.class` writes the class files
instead: `out.class`, and the object, enum, role and runtime classes next to
it (`java -cp . out`).
`int` is exact (BigInteger). The generated Java main calls Okkama main and uses
its result as the process exit status. Several FILE paths form one module
(spec §5.1). A generation error leaves output intact.

Sparrow writes a verified SPB 9 file (register bytecode with the program's
type, object, role and function tables), runnable with `okkama exec FILE.spb`
or the standalone `sparrow FILE.spb`. With -t it holds the test profile, run by
`okkama exec` or `sparrow test`. `sparrow --disassemble` prints its assembly.

Source, lowering and codegen errors exit with code 1, usage errors with code 2,
and input/output errors with code 3. Errors go to stderr; stdout stays empty.
";

const EXEC_HELP: &str = "\
okkama exec — execute Sparrow bytecode

USAGE:
    okkama exec [BUDGETS] <FILE.spb> [ARGS...]

BUDGETS (off by default): --max-steps N, --max-memory SIZE, --max-output SIZE,
--timeout SECONDS (see `okkama help run`); running out exits 1.

Loads and verifies an SPB file, then executes it in the Rust process with
ARGS as the program's arguments; a test-profile file (`build -t`) runs its
tests. Source files and Java are not required. main must return an integer
in 0..255, used as the process exit status; an uncaught failure is reported
on stderr with exit 1. File-format errors exit with 1, usage errors with 2,
and read errors with 3.
";

fn sparrow_exec_command(args: &[String]) -> Outcome {
    let mut limits = sparrow_core::budget::Limits::default();
    let mut args = args;
    while let [option, value, rest @ ..] = args {
        match limits.option(option, value) {
            Ok(true) => args = rest,
            Ok(false) => break,
            Err(message) => return Outcome::usage(&message),
        }
    }
    let Some((path, arguments)) = args.split_first() else {
        return Outcome::usage("exec requires a bytecode file");
    };
    if matches!(path.as_str(), "--help" | "-h") {
        return Outcome::success(EXEC_HELP.to_string());
    }
    if path.starts_with('-') {
        return Outcome::usage("exec requires a file path (stdin and options are not supported)");
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            return Outcome::failure(format!("okkama: cannot read `{path}`: {error}\n"), EXIT_IO);
        }
    };
    // Bound allocation before decoding untrusted length/count fields.
    let mut bytes = Vec::new();
    if let Err(error) = file
        .take(okkama_sparrow::MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        return Outcome::failure(format!("okkama: cannot read `{path}`: {error}\n"), EXIT_IO);
    }
    let program = match okkama_sparrow::decode(&bytes) {
        Ok(program) => program,
        Err(error) => return Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
    };
    if program.tests.is_some() {
        return core_tests(bytes);
    }
    let arguments = arguments.iter().map(|a| a.clone().into_bytes()).collect();
    let (outcome, trace) =
        core_described(sparrow_core::run_described(bytes, arguments, false, core::worker_command(Path::new(path)), limits));
    with_trace(outcome, || sparrow_core::render_trace(&program, &trace))
}

/// A test-profile program on the core: the report goes to stdout.
fn core_tests(bytes: Vec<u8>) -> Outcome {
    match sparrow_core::run_tests(bytes, Box::new(okkama_runtime_core::stdout::Buffered)) {
        Ok(passed) => Outcome { stdout: String::new(), stderr: String::new(), code: u8::from(!passed) },
        Err(error) => Outcome::failure(format!("okkama: {}\n", error.0), EXIT_DIAGNOSTICS),
    }
}

/// `core_outcome` of `run_described`, with the failure's call stack.
fn core_described(
    result: Result<Result<i64, (sparrow_core::Error, Option<sparrow_core::Uncaught>)>, sparrow_core::Error>,
) -> (Outcome, Vec<sparrow_core::Site>) {
    match result {
        Ok(Ok(value)) => (core_outcome(Ok(value)), Vec::new()),
        Ok(Err((error, uncaught))) => (core_outcome(Err(error)), uncaught.map(|u| u.trace).unwrap_or_default()),
        Err(error) => (core_outcome(Err(error)), Vec::new()),
    }
}

/// Stop the process with the interrupt report one second after `flag` is
/// raised, if the run has not stopped by itself by then: the engines poll the
/// flag while they run code and in long natives that can look at it, not in
/// one that cannot (Argon2, a blocking read; #238).
fn watchdog(flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let _ = std::thread::Builder::new().name("okkama-timeout".into()).spawn(move || {
        while !flag.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        okkama_runtime_core::stdout::try_flush();
        let _ = io::stdout().flush();
        let _ = writeln!(io::stderr(), "{}", sparrow_core::budget::Exhausted::Interrupted.message());
        std::process::exit(1);
    });
}

/// main's result on the core: the exit status, or the failure report (exit 1).
fn core_outcome(result: Result<i64, sparrow_core::Error>) -> Outcome {
    match result {
        Ok(value) => match u8::try_from(value) {
            Ok(code) => Outcome { stdout: String::new(), stderr: String::new(), code },
            Err(_) => Outcome::failure(format!("RuntimeError: main() must return an int in 0..255, got {value}\n"), 1),
        },
        Err(error) => Outcome::failure(core::report(error), 1),
    }
}

/// Диагностика отказа программы (код 1) и его место со стеком вызовов;
/// `OKKAMA_TRACE=off` — без них (например, для сравнения движков).
/// The trace is rendered only when it is printed: with `OKKAMA_TRACE=off`
/// a stack of millions of frames costs nothing (#241).
fn with_trace(mut outcome: Outcome, trace: impl FnOnce() -> String) -> Outcome {
    if outcome.code == 1 && std::env::var("OKKAMA_TRACE").as_deref() != Ok("off") {
        outcome.stderr.push_str(&trace());
    }
    outcome
}

/// A finished command: what to print and how to exit.
struct Outcome {
    stdout: String,
    stderr: String,
    code: u8,
}

impl Outcome {
    fn success(stdout: String) -> Self {
        Outcome {
            stdout,
            stderr: String::new(),
            code: 0,
        }
    }

    fn failure(stderr: String, code: u8) -> Self {
        Outcome {
            stdout: String::new(),
            stderr,
            code,
        }
    }

    fn usage(message: &str) -> Self {
        Outcome::failure(
            format!("okkama: {message}\nTry `okkama --help` for more information.\n"),
            EXIT_USAGE,
        )
    }
}

fn main() -> ExitCode {
    #[cfg(feature = "allocation-stats")]
    let _allocation_report = allocation_stats::Report;
    #[cfg(not(feature = "allocation-stats"))]
    sparrow_core::budget::set_memory_probe(sparrow_core::budget::mimalloc_committed);
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Сервер языка владеет stdin/stdout сам.
    if args.first().map(String::as_str) == Some("lsp") {
        return lsp::run();
    }
    let outcome = run(&args);
    // The program's buffered output goes before the command's own and before
    // a failure report (#132). A closed stdout (`okkama ast big.ok | head`) is
    // not an error worth a panic; anything printed so far is all the reader
    // wanted.
    let _ = okkama_runtime_core::stdout::write(outcome.stdout.as_bytes());
    let _ = okkama_runtime_core::stdout::flush();
    let _ = io::stderr().lock().write_all(outcome.stderr.as_bytes());
    ExitCode::from(outcome.code)
}

fn run(args: &[String]) -> Outcome {
    let Some(command) = args.first() else {
        return Outcome::failure(HELP.to_string(), EXIT_USAGE);
    };
    let rest = &args[1..];
    match command.as_str() {
        "-h" | "--help" => Outcome::success(HELP.to_string()),
        "help" => match rest.first().map(String::as_str) {
            None => Outcome::success(HELP.to_string()),
            Some("tokens") => Outcome::success(TOKENS_HELP.to_string()),
            Some("ast") => Outcome::success(AST_HELP.to_string()),
            Some("ir") => Outcome::success(IR_HELP.to_string()),
            Some("build") => Outcome::success(BUILD_HELP.to_string()),
            Some("exec") => Outcome::success(EXEC_HELP.to_string()),
            Some("man") => Outcome::success(man::HELP.to_string()),
            Some(other) => Outcome::usage(&format!("unknown command `{other}`")),
        },
        "-V" | "--version" => Outcome::success(format!("okkama {}\n", env!("CARGO_PKG_VERSION"))),
        // Endpoint conformance-адаптера (`conformance/adapters/PROTOCOL.md`).
        "--ocs-capabilities" => ocs::capabilities(),
        "--ocs-request" | "--ocs-response" => ocs::request(args),
        "tokens" => tokens_command(rest),
        "ast" => ast_command(rest),
        "check" => check_command(rest),
        "fmt" => fmt_command(rest),
        "env" => env_command(rest),
        "lint" => lint_command(rest),
        "man" => man::command(rest),
        "ir" => ir_command(rest),
        "build" => build_command(rest),
        "exec" => sparrow_exec_command(rest),
        // Child side of a process worker of the core (`core.rs`).
        "--worker" => match core::worker(rest) {
            Ok(()) => Outcome { stdout: String::new(), stderr: String::new(), code: 0 },
            Err(message) => Outcome::failure(format!("okkama worker: {message}\n"), 1),
        },
        "run" => run_command(rest),
        // Как у Perl-CLI: `okkama [-t] program.ok args…`.
        "-t" => run_command(args),
        other if other.ends_with(".ok") => run_command(args),
        other => Outcome::usage(&format!("unknown command `{other}`")),
    }
}

/// Command families have separate flags while sharing input/help handling.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CommandOptions {
    Basic,
    Ast,
    /// `check` и `ir`: программа из одного или нескольких файлов.
    Program,
    Build,
}

#[derive(Default)]
struct CommandLine {
    input: Option<String>,
    /// Остальные файлы многофайлового модуля (`check`, `ir`, `build`).
    more_inputs: Vec<String>,
    /// `--tests`: тестовый режим (§14) — тесты корня вместо `main`.
    tests: bool,
    format: Option<String>,
    spans: bool,
    mode: Option<String>,
    prune_tests: bool,
    backend: Option<String>,
    output: Option<String>,
    /// `build --budgeted`: JVM-классы считают шаги (`-Dokkama.steps=N`, #237).
    budgeted: bool,
    help: bool,
}

fn parse_command_line(args: &[String], options: CommandOptions) -> Result<CommandLine, String> {
    let mut line = CommandLine::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (flag, inline_value) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arg.as_str(), None),
        };
        let mut value_for = |name: &str| -> Result<String, String> {
            match inline_value.clone() {
                Some(value) => Ok(value),
                None => iter
                    .next()
                    .cloned()
                    .ok_or_else(|| format!("option `{name}` needs a value")),
            }
        };
        match flag {
            "-h" | "--help" => line.help = true,
            "--tests" | "-t"
                if matches!(options, CommandOptions::Program | CommandOptions::Build) =>
            {
                line.tests = true;
            }
            "--format" if options != CommandOptions::Build => {
                line.format = Some(value_for("--format")?);
            }
            "--spans" if options == CommandOptions::Ast => line.spans = true,
            "--mode" if options == CommandOptions::Ast => line.mode = Some(value_for("--mode")?),
            "--prune-tests" if options == CommandOptions::Ast => line.prune_tests = true,
            "--target" | "--backend" if options == CommandOptions::Build => {
                let value = value_for(flag)?;
                if line.backend.as_ref().is_some_and(|old| old != &value) {
                    return Err("conflicting target selections".into());
                }
                line.backend = Some(value);
            }
            "-o" | "--output" if options == CommandOptions::Build => {
                line.output = Some(value_for(flag)?);
            }
            "--budgeted" if options == CommandOptions::Build => line.budgeted = true,
            "-" => set_input(&mut line, arg, options)?,
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            _ => set_input(&mut line, arg, options)?,
        }
    }
    Ok(line)
}

fn set_input(line: &mut CommandLine, arg: &str, options: CommandOptions) -> Result<(), String> {
    let program = matches!(options, CommandOptions::Program | CommandOptions::Build);
    if program && line.input.is_some() && arg != "-" && line.input.as_deref() != Some("-") {
        line.more_inputs.push(arg.to_string());
        return Ok(());
    }
    if line.input.is_some() {
        return Err(format!(
            "unexpected extra argument `{arg}`; only one input is accepted"
        ));
    }
    line.input = Some(arg.to_string());
    Ok(())
}

/// Reads the input as a source file, or produces the failing outcome.
fn load(input: Option<&str>) -> Result<SourceFile, Outcome> {
    let Some(input) = input else {
        return Err(Outcome::usage("missing input file (use `-` for stdin)"));
    };
    let (name, bytes) = if input == "-" {
        let mut bytes = Vec::new();
        if let Err(error) = io::stdin().lock().read_to_end(&mut bytes) {
            return Err(Outcome::failure(
                format!("okkama: cannot read standard input: {error}\n"),
                EXIT_IO,
            ));
        }
        ("<stdin>".to_string(), bytes)
    } else {
        match std::fs::read(input) {
            Ok(bytes) => (input.to_string(), bytes),
            Err(error) => {
                return Err(Outcome::failure(
                    format!("okkama: cannot read `{input}`: {error}\n"),
                    EXIT_IO,
                ));
            }
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let offset = error.utf8_error().valid_up_to();
            return Err(Outcome::failure(
                format!(
                    "{name}: lexical error: Source is not valid UTF-8 (invalid byte at offset {offset}).\n"
                ),
                EXIT_DIAGNOSTICS,
            ));
        }
    };
    SourceFile::new(name, text)
        .map_err(|error| Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS))
}

/// Все файлы программы: первый (может быть `-`), затем остальные.
fn load_all(line: &CommandLine) -> Result<Vec<SourceFile>, Outcome> {
    let mut sources = vec![load(line.input.as_deref())?];
    for input in &line.more_inputs {
        sources.push(load(Some(input))?);
    }
    // The files of one module are taken in name order, so the order of the
    // arguments changes nothing — which declaration is first, the default
    // entry file, the order of tests and of diagnostics (H-16).
    sources.sort_by(|a, b| a.name().cmp(b.name()));
    Ok(sources)
}

/// Разбор файлов программы: без тестовых объявлений или, с `--tests`, с
/// тестами и fakes (§14).
fn parse_all(sources: &[SourceFile], tests: bool) -> Result<Vec<ast::Program>, Outcome> {
    sources
        .iter()
        .map(|source| {
            syntax::parse(source, &ParseOptions::default())
                .map(|program| {
                    if tests {
                        syntax::prune::desugar_test_items(program)
                    } else {
                        syntax::prune::strip_test_items(program)
                    }
                })
                .map_err(|diagnostics| diagnostics_outcome(&diagnostics, source))
        })
        .collect()
}

fn loader(tests: bool) -> FileLoader {
    let loader = FileLoader::from_env();
    if tests { loader.for_tests() } else { loader }
}

fn root_files<'a>(sources: &'a [SourceFile], programs: &'a [ast::Program]) -> Vec<RootFile<'a>> {
    sources
        .iter()
        .zip(programs)
        .map(|(source, program)| RootFile { program, source })
        .collect()
}

fn diagnostics_outcome(diagnostics: &[Diagnostic], source: &SourceFile) -> Outcome {
    Outcome::failure(render_all(diagnostics, source), EXIT_DIAGNOSTICS)
}

fn tokens_command(args: &[String]) -> Outcome {
    let line = match parse_command_line(args, CommandOptions::Basic) {
        Ok(line) => line,
        Err(message) => return Outcome::usage(&message),
    };
    if line.help {
        return Outcome::success(TOKENS_HELP.to_string());
    }
    let json = match line.format.as_deref() {
        None | Some("text") => false,
        Some("json") => true,
        Some(other) => {
            return Outcome::usage(&format!(
                "unknown token format `{other}` (use text or json)"
            ));
        }
    };
    let source = match load(line.input.as_deref()) {
        Ok(source) => source,
        Err(outcome) => return outcome,
    };
    let lexed = lexer::lex(source.text());
    if lexed.has_errors() {
        return diagnostics_outcome(&lexed.diagnostics, &source);
    }
    let out = if json {
        tokens_json(&lexed.tokens, &source)
    } else {
        tokens_text(&lexed.tokens, &source)
    };
    Outcome::success(out)
}

fn ast_command(args: &[String]) -> Outcome {
    let line = match parse_command_line(args, CommandOptions::Ast) {
        Ok(line) => line,
        Err(message) => return Outcome::usage(&message),
    };
    if line.help {
        return Outcome::success(AST_HELP.to_string());
    }
    let format = match line.format.as_deref() {
        None | Some("tree") => Format::Tree,
        Some("json") => Format::Json,
        Some(other) => {
            return Outcome::usage(&format!("unknown AST format `{other}` (use tree or json)"));
        }
    };
    let mode = match line.mode.as_deref() {
        None | Some("full") => ParseMode::Full,
        Some("production") => ParseMode::Production,
        Some(other) => {
            return Outcome::usage(&format!("unknown mode `{other}` (use full or production)"));
        }
    };
    let source = match load(line.input.as_deref()) {
        Ok(source) => source,
        Err(outcome) => return outcome,
    };
    let options = ParseOptions {
        mode,
        ..ParseOptions::default()
    };
    let program = match syntax::parse(&source, &options) {
        Ok(program) => program,
        Err(diagnostics) => return diagnostics_outcome(&diagnostics, &source),
    };
    let program = if line.prune_tests {
        prune::strip_test_items(program)
    } else {
        program
    };
    let options = PrintOptions { spans: line.spans };
    Outcome::success(print_program(&program, &source, format, options))
}

fn ir_command(args: &[String]) -> Outcome {
    // `--passes LIST` / `--after PASS`: the IR after some passes (H-19).
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--passes" | "--after" => {
                let Some(value) = iter.next() else {
                    return Outcome::usage(&format!("{arg} needs a value"));
                };
                let spec = if arg == "--passes" {
                    value.clone()
                } else {
                    let Some(at) = lowering::PASSES.iter().position(|pass| pass == value) else {
                        return Outcome::usage(&format!("unknown pass `{value}` (passes: {})", lowering::PASSES.join(", ")));
                    };

                    lowering::PASSES[..=at].join(",")
                };
                lowering::select_passes(&spec);
            }
            _ => rest.push(arg.clone()),
        }
    }
    let args = &rest[..];
    let line = match parse_command_line(args, CommandOptions::Program) {
        Ok(line) => line,
        Err(message) => return Outcome::usage(&message),
    };
    if line.help {
        return Outcome::success(IR_HELP.to_string());
    }
    match line.format.as_deref() {
        None | Some("text") => {}
        Some(other) => {
            return Outcome::usage(&format!("unknown IR format `{other}` (use text)"));
        }
    }
    let sources = match load_all(&line) {
        Ok(sources) => sources,
        Err(outcome) => return outcome,
    };
    match lower_source(&sources, line.tests) {
        Ok((module, warnings)) => Outcome {
            stdout: module.to_string(),
            stderr: warnings,
            code: 0,
        },
        Err(outcome) => outcome,
    }
}

/// `okkama check`: семантический анализ программы и её импортов без lowering.
/// Печатаются все ошибки, а не только первая.
const FMT_HELP: &str = "okkama fmt — format Okkama source files

USAGE:
    okkama fmt [-w | --check] <FILE|->...

Prints the formatted text of one FILE (or `-`, standard input) to standard
output. `-w` rewrites every FILE in place; `--check` changes nothing and lists
the files that are not formatted (exit status 1 if any). The style is
perlstyle: 4-space indentation, one statement or member per line, `else` and
`catch` on their own line, spaces around binary operators and after commas.
Only whitespace changes: tokens and comments are kept and verified.
";

const LINT_HELP: &str = "okkama lint — errors, warnings and lint findings

USAGE:
    okkama lint <FILE>...

Checks each FILE like `okkama check` (imports included; files with tests in
test mode) and adds the lint rules: unused-import, unused-let,
unused-hidden, unreachable, snake-case. Prints
`file:line:column: severity[rule]: message`; exit status 1 if anything was
reported. A `let` whose name starts with `_` is never reported as unused.
";

fn lint_command(args: &[String]) -> Outcome {
    let mut files = Vec::new();
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Outcome::success(LINT_HELP.to_string()),
            other if other.starts_with('-') => {
                return Outcome::usage(&format!("unknown lint option `{other}`"));
            }
            other => files.push(other.to_string()),
        }
    }
    if files.is_empty() {
        return Outcome::usage("lint needs a FILE");
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut code = 0;
    for file in &files {
        let path = std::fs::canonicalize(file).unwrap_or_else(|_| PathBuf::from(file));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                stderr.push_str(&format!("okkama: cannot read `{file}`: {error}\n"));
                code = EXIT_IO;
                continue;
            }
        };
        let source = match SourceFile::new(path.display().to_string(), text) {
            Ok(source) => source,
            Err(error) => {
                stderr.push_str(&format!("okkama: {error}\n"));
                code = EXIT_IO;
                continue;
            }
        };
        let test_mode = okkama_compiler::ide::has_tests(&source);
        let mut loader = loader(test_mode);
        let inspection = lowering::inspect(&source, &mut loader, test_mode);
        for found in &inspection.diagnostics {
            let shown = if found.path == source.name() {
                file.clone()
            } else {
                found.path.clone()
            };
            let severity = match found.severity {
                okkama_compiler::ide::Severity::Error => "error",
                okkama_compiler::ide::Severity::Warning => "warning",
            };
            let rule = found
                .code
                .as_ref()
                .map_or(String::new(), |code| format!("[{code}]"));
            stdout.push_str(&format!(
                "{shown}:{}:{}: {severity}{rule}: {}\n",
                found.range.start.line + 1,
                found.range.start.character + 1,
                found.message
            ));
            if code == 0 {
                code = 1;
            }
        }
    }
    Outcome {
        stdout,
        stderr,
        code,
    }
}

fn fmt_command(args: &[String]) -> Outcome {
    let mut write = false;
    let mut check = false;
    let mut files = Vec::new();
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Outcome::success(FMT_HELP.to_string()),
            "-w" | "--write" => write = true,
            "--check" => check = true,
            other if other.starts_with('-') && other != "-" => {
                return Outcome::usage(&format!("unknown fmt option `{other}`"));
            }
            other => files.push(other.to_string()),
        }
    }
    if files.is_empty() {
        return Outcome::usage("fmt needs a FILE (or `-` for standard input)");
    }
    if write && check {
        return Outcome::usage("use either -w or --check");
    }
    if !write && !check && files.len() > 1 {
        return Outcome::usage(
            "printing to standard output takes one FILE; use -w or --check for several",
        );
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut failed = false;
    let mut io_failed = false;
    let mut unformatted = false;
    for file in &files {
        let text = if file == "-" {
            let mut text = String::new();
            if let Err(error) = io::stdin().read_to_string(&mut text) {
                return Outcome::failure(
                    format!("okkama: cannot read standard input: {error}\n"),
                    EXIT_DIAGNOSTICS,
                );
            }
            text
        } else {
            match std::fs::read_to_string(file) {
                Ok(text) => text,
                Err(error) => {
                    stderr.push_str(&format!("okkama: cannot read `{file}`: {error}\n"));
                    io_failed = true;
                    continue;
                }
            }
        };
        let name = if file == "-" {
            "<stdin>"
        } else {
            file.as_str()
        };
        let source = match SourceFile::new(name, text.clone()) {
            Ok(source) => source,
            Err(error) => {
                stderr.push_str(&format!("okkama: {error}\n"));
                failed = true;
                continue;
            }
        };
        let formatted = match okkama_compiler::syntax::format::format(&source) {
            Ok(formatted) => formatted,
            Err(okkama_compiler::syntax::format::FormatError::Syntax(diagnostics)) => {
                stderr.push_str(&render_all(&diagnostics, &source));
                failed = true;
                continue;
            }
            Err(okkama_compiler::syntax::format::FormatError::Internal(message)) => {
                stderr.push_str(&format!(
                    "{name}: formatter error (file left unchanged): {message}\n"
                ));
                failed = true;
                continue;
            }
        };
        if check {
            if formatted != text {
                stdout.push_str(&format!("{name}\n"));
                unformatted = true;
            }
        } else if write && file != "-" {
            if formatted != text
                && let Err(error) = std::fs::write(file, &formatted) {
                    stderr.push_str(&format!("okkama: cannot write `{file}`: {error}\n"));
                    io_failed = true;
                }
        } else {
            stdout.push_str(&formatted);
        }
    }
    Outcome {
        stdout,
        stderr,
        code: if io_failed {
            EXIT_IO
        } else if failed {
            EXIT_DIAGNOSTICS
        } else if unformatted {
            1
        } else {
            0
        },
    }
}

fn check_command(args: &[String]) -> Outcome {
    let line = match parse_command_line(args, CommandOptions::Program) {
        Ok(line) => line,
        Err(message) => return Outcome::usage(&message),
    };
    if line.help {
        return Outcome::success(HELP.to_string());
    }
    let sources = match load_all(&line) {
        Ok(sources) => sources,
        Err(outcome) => return outcome,
    };
    let programs = match parse_all(&sources, line.tests) {
        Ok(programs) => programs,
        Err(outcome) => return outcome,
    };
    let source = &sources[0];
    let mut loader = loader(line.tests);
    let roots = root_files(&sources, &programs);
    let checked = if line.tests {
        lowering::check_linked_tests(&roots, &mut loader)
    } else {
        lowering::check_linked_files(&roots, &mut loader)
    };
    match checked {
        Ok(warnings) => Outcome {
            stdout: String::new(),
            stderr: render_warnings(source, &warnings),
            code: 0,
        },
        Err(Err(text)) => Outcome::failure(text, EXIT_DIAGNOSTICS),
        Err(Ok(errors)) => {
            let mut text = String::new();
            for found in errors {
                let file = found.source.as_ref().unwrap_or(source);
                let location = file.location(found.error.span.start);
                let kind = if found.error.unsupported {
                    "analysis limitation"
                } else {
                    "semantic error"
                };
                text.push_str(&format!(
                    "{}:{}:{}: {kind}: {}\n",
                    file.name(),
                    location.line,
                    location.column,
                    found.error.message
                ));
            }
            Outcome::failure(text, EXIT_DIAGNOSTICS)
        }
    }
}

/// The same source-to-IR pipeline serves both inspection and artifact builds.
///
/// IR describes the production program: test and fake declarations are removed
/// by the explicit pruning pass before lowering, never skipped by it.
fn lower_source(sources: &[SourceFile], tests: bool) -> Result<(ir::Module, String), Outcome> {
    let programs = parse_all(sources, tests)?;
    let source = &sources[0];
    let mut loader = loader(tests);
    let roots = root_files(sources, &programs);
    let lowered = if tests {
        lowering::lower_linked_tests(&roots, &mut loader)
    } else {
        lowering::lower_linked_files(&roots, &mut loader)
    };
    lowered
        .map(|(module, warnings)| (module, render_warnings(source, &warnings)))
        .map_err(|error| match error {
            LinkError::Lowering {
                source: imported,
                error,
            } => {
                let kind = match error {
                    LoweringError::Unsupported { .. } => "lowering error",
                    LoweringError::Invalid { .. } => "semantic error",
                };
                ir_error_outcome(
                    imported.as_ref().unwrap_or(source),
                    error.span(),
                    kind,
                    error.message(),
                )
            }
            LinkError::Load(text) => Outcome::failure(text, EXIT_DIAGNOSTICS),
        })
}

/// Импорты из файлов, как в Perl: сначала рядом с импортирующим файлом и в
/// корне его пакета (`blade.toml`), затем системные каталоги
/// `OKKAMA_IMPORT_DIRS` (через `:`), затем std (`OKKAMA_STDLIB_DIR`, по
/// умолчанию `/usr/local/lib/okkama/std`). `pkg/…` — зависимости из `pkg/`
/// корня проекта; файлы std ищут импорты только в std.
struct FileLoader {
    import_dirs: Vec<PathBuf>,
    stdlib: PathBuf,
    /// Тестовый режим (§14): импорты разбираются с fakes и тестами.
    test_mode: bool,
    /// Несохранённые тексты открытых в редакторе файлов (`okkama lsp`), по
    /// каноническому пути.
    overlay: std::collections::HashMap<PathBuf, String>,
}

impl FileLoader {
    /// Std из запроса conformance; каталоги импорта — из окружения.
    fn with_stdlib(stdlib: PathBuf) -> Self {
        Self {
            stdlib,
            ..Self::from_env()
        }
    }

    fn from_env() -> Self {
        let import_dirs = std::env::var("OKKAMA_IMPORT_DIRS")
            .map(|dirs| {
                dirs.split(':')
                    .filter(|dir| !dir.is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            import_dirs,
            stdlib: stdlib_dir(),
            test_mode: false,
            overlay: std::collections::HashMap::new(),
        }
    }

    fn for_tests(mut self) -> Self {
        self.test_mode = true;
        self
    }
}

impl ModuleLoader for FileLoader {
    fn load(&mut self, path: &str, importer: &str) -> Result<Option<LoadedModule>, String> {
        let (file, dirs) = self.candidates(path, importer);
        self.load_from(&file, &dirs)
    }

    fn resolve(&mut self, path: &str, importer: &str) -> Result<Option<String>, String> {
        let (file, dirs) = self.candidates(path, importer);
        Ok(locate(&file, &dirs).map(|found| found.display().to_string()))
    }

    fn resolve_std(&mut self, path: &str, _importer: &str) -> Result<Option<String>, String> {
        Ok(locate(&format!("{path}.ok"), std::slice::from_ref(&self.stdlib)).map(|found| found.display().to_string()))
    }

    /// В blade-проекте `pkg/…` — только прямые зависимости пакета, которому
    /// принадлежит импортирующий файл; вне проекта `pkg/` — обычный каталог.
    fn check_import(&mut self, path: &str, importer: &str) -> Result<(), String> {
        if !manifest::is_dependency_path(path) {
            return Ok(());
        }
        let importer = importer_path(importer);
        if self.in_std(&importer) {
            return Ok(());
        }
        let Some(package) = manifest::package_root(&importer) else {
            return Ok(());
        };
        let found = manifest::Manifest::read(&package)?;
        if found.dependency_of(path).is_none() {
            let name = path.split('/').nth(1).unwrap_or_default();
            return Err(format!(
                "\"{name}\" is not a dependency: add it to [dependencies] in {}",
                package.join(manifest::MANIFEST).display()
            ));
        }
        Ok(())
    }

    fn in_project(&mut self, file: &str) -> bool {
        let file = importer_path(file);
        !self.in_std(&file) && manifest::package_root(&file).is_some()
    }

    /// `std.…`: модуль только из std, а не из каталога импорта или программы.
    fn load_std(&mut self, path: &str, _importer: &str) -> Result<Option<LoadedModule>, String> {
        let stdlib = self.stdlib.clone();
        self.load_from(&format!("{path}.ok"), &[stdlib])
    }
}

/// Путь импортирующего файла; у виртуальных (`<stdin>`, `<repl>`) — в текущем каталоге.
fn importer_path(importer: &str) -> PathBuf {
    let path = if importer.starts_with('<') {
        Path::new(".").join("<input>")
    } else {
        PathBuf::from(importer)
    };
    let absolute = std::path::absolute(&path).unwrap_or(path);
    match (absolute.parent(), absolute.file_name()) {
        (Some(dir), Some(name)) => std::fs::canonicalize(dir)
            .map(|dir| dir.join(name))
            .unwrap_or(absolute),
        _ => absolute,
    }
}

/// The first file `file` of the directories `dirs`, canonical.
fn locate(file: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let candidate = dirs.iter().map(|dir| dir.join(file)).find(|candidate| candidate.is_file())?;
    Some(std::fs::canonicalize(&candidate).unwrap_or(candidate))
}

impl FileLoader {
    /// The file name of import `path` and the directories to look in.
    fn candidates(&self, path: &str, importer: &str) -> (String, Vec<PathBuf>) {
        let file = if path.ends_with(".ok") {
            path.to_string()
        } else {
            format!("{path}.ok")
        };
        // A module never imports itself: `"json"` from app/json.ok skips
        // app/json.ok and finds std/json.
        let itself = std::fs::canonicalize(importer_path(importer)).ok();
        let dirs = self
            .search_dirs(path, importer)
            .into_iter()
            .filter(|dir| itself.is_none() || std::fs::canonicalize(dir.join(&file)).ok() != itself)
            .collect();
        (file, dirs)
    }

    fn in_std(&self, file: &Path) -> bool {
        let stdlib = std::fs::canonicalize(&self.stdlib).unwrap_or_else(|_| self.stdlib.clone());
        file.starts_with(stdlib)
    }

    /// Каталоги поиска импорта `path` из файла `importer`, по порядку.
    fn search_dirs(&self, path: &str, importer: &str) -> Vec<PathBuf> {
        let importer = importer_path(importer);
        let base = importer
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        // Std imports name modules from the std root (`"template/model"`), so
        // the root comes first: `"json"` from std/template/json.ok is std/json.
        if self.in_std(&importer) {
            return vec![self.stdlib.clone(), base];
        }
        let package = manifest::package_root(&importer);
        if let Some(package) = &package
            && manifest::is_dependency_path(path) {
                return vec![manifest::project_root(package)];
            }
        let mut dirs = vec![base];
        dirs.extend(package.filter(|package| !dirs.contains(package)));
        dirs.extend(self.import_dirs.iter().cloned());
        dirs.push(self.stdlib.clone());
        dirs
    }

    /// Первый файл `file` из каталогов `dirs`, разобранный как модуль.
    fn load_from(&mut self, file: &str, dirs: &[PathBuf]) -> Result<Option<LoadedModule>, String> {
        if let Some(canonical) = locate(file, dirs) {
            let name = canonical.display().to_string();
            let text = match self.overlay.get(&canonical) {
                Some(text) => text.clone(),
                None => std::fs::read_to_string(&canonical)
                    .map_err(|error| format!("okkama: cannot read import `{name}`: {error}\n"))?,
            };
            let source = SourceFile::new(name.clone(), text)
                .map_err(|error| format!("okkama: {error}\n"))?;
            // Production — без тестов (§14); тестовый режим — с fakes импортов.
            let options = ParseOptions {
                mode: if self.test_mode {
                    ParseMode::Full
                } else {
                    ParseMode::Production
                },
                ..ParseOptions::default()
            };
            let program = syntax::parse(&source, &options)
                .map_err(|diagnostics| render_all(&diagnostics, &source))?;
            let program = if self.test_mode {
                prune::desugar_test_items(program)
            } else {
                prune::strip_test_items(program)
            };
            return Ok(Some(LoadedModule {
                id: name,
                program,
                source,
            }));
        }
        Ok(None)
    }
}

fn build_command(args: &[String]) -> Outcome {
    let line = match parse_command_line(args, CommandOptions::Build) {
        Ok(line) => line,
        Err(message) => return Outcome::usage(&message),
    };
    if line.help {
        return Outcome::success(BUILD_HELP.to_string());
    }
    let backend = match line.backend.as_deref() {
        Some(name) => match Backend::from_name(name) {
            Some(backend) => backend,
            None => {
                return Outcome::usage(&format!("unknown backend `{name}` (use jvm or sparrow)"));
            }
        },
        None => {
            return Outcome::usage("missing --target (use --target jvm or --target sparrow)");
        }
    };
    let output = match line.output.as_deref() {
        Some("") => return Outcome::usage("output file path must not be empty"),
        Some(output) => output,
        None => return Outcome::usage("missing output file (use -o FILE)"),
    };
    let Some(artifact_name) = Path::new(output).file_stem().and_then(|name| name.to_str()) else {
        return Outcome::usage("output path must contain a file name");
    };
    let sources = match load_all(&line) {
        Ok(sources) => sources,
        Err(outcome) => return outcome,
    };
    let (module, warnings) = match lower_source(&sources, line.tests) {
        Ok(lowered) => lowered,
        Err(outcome) => return outcome,
    };
    if line.budgeted && backend != Backend::Jvm {
        return Outcome::usage("--budgeted applies to --target jvm (Sparrow bytecode always honours budgets)");
    }
    let options = CodegenOptions {
        artifact_name: artifact_name.to_string(),
        budgeted: line.budgeted,
    };
    let artifact = match codegen::generate(&module, backend, &options) {
        Ok(artifact) => with_native_libraries(&module, artifact),
        Err(error) => {
            return Outcome::failure(
                format!("okkama: codegen error: {error}\n"),
                EXIT_DIAGNOSTICS,
            );
        }
    };
    // Open the destinations only after generation succeeds. Backend errors must
    // leave a previous artifact intact and must not create an empty class file.
    if output.ends_with(".jar") {
        let bytes = match artifact.jar(artifact_name) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Outcome::failure(
                    format!("okkama: codegen error: {error}\n"),
                    EXIT_DIAGNOSTICS,
                );
            }
        };
        if let Err(error) = std::fs::write(output, bytes) {
            return Outcome::failure(
                format!("okkama: cannot write `{output}`: {error}\n"),
                EXIT_IO,
            );
        }
        return Outcome {
            stdout: String::new(),
            stderr: warnings,
            code: 0,
        };
    }
    // The main file goes to `-o`; supporting files (JVM object and role
    // classes, the okkama.rt runtime) are written next to it under their own
    // relative names.
    let directory = Path::new(output).parent().unwrap_or(Path::new(""));
    for (index, file) in artifact.files.iter().enumerate() {
        let path = if index == 0 {
            Path::new(output).to_path_buf()
        } else {
            directory.join(&file.name)
        };
        // Runtime classes live in package subdirectories (`okkama/rt/`).
        if let Some(parent) = path.parent().filter(|_| index > 0)
            && let Err(error) = std::fs::create_dir_all(parent) {
                return Outcome::failure(
                    format!("okkama: cannot create `{}`: {error}\n", parent.display()),
                    EXIT_IO,
                );
            }
        if let Err(error) = std::fs::write(&path, &file.bytes) {
            return Outcome::failure(
                format!("okkama: cannot write `{}`: {error}\n", path.display()),
                EXIT_IO,
            );
        }
    }
    Outcome {
        stdout: String::new(),
        stderr: warnings,
        code: 0,
    }
}

/// `okkama run`: Sparrow (JIT) в процессе; `OKKAMA_ENGINE` меняет умолчание;
/// --engine jvm собирает классы и запускает java с потоками и кодом выхода
/// программы. Файлы модуля — первый и следующие
/// за ним существующие `.ok`; остальное (после необязательного `--`) —
/// аргументы программы.
fn run_command(args: &[String]) -> Outcome {
    let mut rest = args;
    let mut tests = false;
    let mut filters: Vec<String> = Vec::new();
    let default = std::env::var("OKKAMA_ENGINE").unwrap_or_default();
    let mut limits = sparrow_core::budget::Limits::default();
    let mut budgeted = false;
    let mut engine = match default.as_str() {
        "sparrow" => "sparrow",
        "ir" => "ir",
        "jvm" => "jvm",
        _ => "sparrow",
    };
    while let Some(first) = rest.first() {
        match first.as_str() {
            // Служебный ключ процесса-воркера `spawn`: корень std родителя.
            "--stdlib" => {
                let Some(value) = rest.get(1) else {
                    return Outcome::usage("--stdlib needs a directory");
                };
                let _ = STDLIB_OVERRIDE.set(PathBuf::from(value));
                rest = &rest[1..];
            }
            "--engine" => {
                let Some(value) = rest.get(1) else {
                    return Outcome::usage("--engine needs a value: sparrow or jvm");
                };
                if value == "ir" {
                    return Outcome::usage("the IR engine was removed (#584); Sparrow is the only Okkama engine (use --engine sparrow); JVM remains a legacy backend until #585");
                }
                if !matches!(value.as_str(), "sparrow" | "jvm") {
                    return Outcome::usage("unknown engine (use sparrow or jvm)");
                }
                engine = value;
                rest = &rest[1..];
            }
            "-t" | "--tests" => tests = true,
            "--filter" => {
                let Some(value) = rest.get(1) else {
                    return Outcome::usage("--filter needs a text to look for in test names");
                };
                filters.push(value.clone());
                rest = &rest[1..];
            }
            "-h" | "--help" => return Outcome::success(HELP.to_string()),
            "--" => {
                rest = &rest[1..];
                break;
            }
            budget if budget.starts_with("--max-") || budget == "--timeout" => {
                let Some(value) = rest.get(1) else {
                    return Outcome::usage(&format!("{budget} needs a value"));
                };
                match limits.option(budget, value) {
                    Ok(true) => budgeted = true,
                    Ok(false) => return Outcome::usage(&format!("unknown option `{budget}`")),
                    Err(message) => return Outcome::usage(&message),
                }
                rest = &rest[1..];
            }
            other if other.starts_with('-') && other != "-" => {
                return Outcome::usage(&format!("unknown option `{other}`"));
            }
            _ => break,
        }
        rest = &rest[1..];
    }
    if engine == "ir" {
        return Outcome::usage("the IR engine was removed (#584); Sparrow is the only Okkama engine (use --engine sparrow); JVM remains a legacy backend until #585");
    }
    if !filters.is_empty() && !tests {
        return Outcome::usage("--filter selects tests: use it with -t");
    }
    if budgeted && !matches!(engine, "sparrow" | "jvm") {
        return Outcome::usage("execution budgets (--max-*, --timeout) need --engine sparrow or jvm");
    }
    let Some((input, mut program_args)) = rest.split_first() else {
        return Outcome::usage("missing input file");
    };
    let mut line = CommandLine {
        input: Some(input.clone()),
        tests,
        ..CommandLine::default()
    };
    while let Some((next, tail)) = program_args.split_first() {
        if !(next.ends_with(".ok") && Path::new(next).is_file()) {
            break;
        }
        line.more_inputs.push(next.clone());
        program_args = tail;
    }
    if program_args.first().map(String::as_str) == Some("--") {
        program_args = &program_args[1..];
    }
    let sources = match load_all(&line) {
        Ok(sources) => sources,
        Err(outcome) => return outcome,
    };
    let (mut module, warnings) = match lower_source(&sources, line.tests) {
        Ok(lowered) => lowered,
        Err(outcome) => return outcome,
    };
    let _ = io::stderr().lock().write_all(warnings.as_bytes());
    if let Some(tests) = module.tests.as_mut()
        && !filters.is_empty()
    {
        // `--filter` (#277): the tests whose names contain one of the texts.
        tests.retain(|(name, _)| filters.iter().any(|text| name.contains(text.as_str())));
        if tests.is_empty() {
            let texts: Vec<String> = filters.iter().map(|text| format!("{text:?}")).collect();
            return Outcome::failure(
                format!("okkama: no test name contains {}\n", texts.join(" or ")),
                EXIT_DIAGNOSTICS,
            );
        }
    }
    if engine != "jvm" && std::env::var_os("OKKAMA_ICU_LIBDIR").is_none() {
        // Как у JVM-запуска: программа видит закреплённую ICU в окружении.
        if let Some(icu) = pinned_icu() {
            sparrow_core::default_environment("OKKAMA_ICU_LIBDIR", &icu.display().to_string());
        }
    }
    if engine == "sparrow" {
        let program = match codegen::sparrow::generate(&module) {
            Ok(program) => program,
            Err(error) => return Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
        };
        let bytes = match okkama_sparrow::encode(&program) {
            Ok(bytes) => bytes,
            Err(error) => return Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
        };
        if tests {
            return core_tests(bytes);
        }
        let (command, _file) = match core::workers_for(&program, &bytes) {
            Ok(workers) => workers,
            Err(message) => return Outcome::failure(format!("okkama: {message}\n"), EXIT_IO),
        };
        let arguments = program_args.iter().map(|a| a.clone().into_bytes()).collect();
        if let Some(flag) = &limits.interrupt {
            watchdog(flag.clone());
        }
        // Номера функций и инструкций ядра совпадают с OIR: место — по исходнику.
        let (outcome, trace) = core_described(sparrow_core::run_described(bytes, arguments, false, command, limits));
        return with_trace(outcome, || trace::render_trace(&module, &trace));
    }
    let options = CodegenOptions {
        artifact_name: "Main".to_string(),
        budgeted: limits.steps.is_some(),
    };
    let artifact = match codegen::generate(&module, Backend::Jvm, &options) {
        Ok(artifact) => with_native_libraries(&module, artifact),
        Err(error) => {
            return Outcome::failure(
                format!("okkama: codegen error: {error}\n"),
                EXIT_DIAGNOSTICS,
            );
        }
    };
    let classpath = match cached_classpath(&artifact.files) {
        Ok(classpath) => classpath,
        Err(text) => return Outcome::failure(text, EXIT_IO),
    };
    // The runtime classes are Java 22 (FFM natives); an older `java` would
    // only fail with UnsupportedClassVersionError.
    let major = java_major();
    if (1..22).contains(&major) {
        return Outcome::failure(
            format!("okkama: the JVM engine needs JDK 22 or newer; `java` is Java {major}\n"),
            EXIT_IO,
        );
    }
    let args: Vec<&str> = program_args.iter().map(String::as_str).collect();
    let mut command = java_command(&classpath, "Main", &args, &jvm_limits(limits.steps, limits.memory));
    if limits.output.is_some() || limits.interrupt.is_some() {
        return jvm_budgeted(&mut command, limits.output, limits.interrupt.clone());
    }
    exec_or_wait(&mut command)
}

/// Steps and memory on the JVM (#237): the program counts steps itself
/// (classes generated with `budgeted`, `Rt.step`), and the memory limit is the
/// Java heap (`-Xmx`, which also counts the runtime's own objects) and the
/// bound of every thread stack; `okkama.rt.Rt` turns running out of either
/// into the core's report.
fn jvm_limits(steps: Option<u64>, memory: Option<u64>) -> Vec<String> {
    let mut options = Vec::new();
    if let Some(steps) = steps {
        options.push(format!("-Dokkama.steps={steps}"));
    }
    if let Some(memory) = memory {
        // The JVM refuses a heap below 2 MiB and an initial heap above the maximum.
        let heap = memory.max(2 << 20);
        options.push(format!("-Xmx{heap}"));
        options.push(format!("-Xms{}", heap.min(8 << 20)));
        options.push(format!("-Dokkama.memory={heap}"));
    }
    options
}

/// The JVM under an output limit or a timeout (#71): `java` as a child whose
/// stdout and stderr pass through counters; past the limit, or when the
/// timer raises `interrupt`, it is killed with the core's report.
fn jvm_budgeted(
    command: &mut std::process::Command,
    output: Option<u64>,
    interrupt: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Outcome {
    use std::io::Read;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return Outcome::failure(format!("okkama: cannot start java: {error}\n"), EXIT_IO),
    };
    let left = Arc::new(AtomicU64::new(output.unwrap_or(u64::MAX)));
    let exceeded = Arc::new(AtomicBool::new(false));
    let copy = |mut from: Box<dyn Read + Send>, to: fn() -> Box<dyn Write>| {
        let (left, exceeded) = (left.clone(), exceeded.clone());
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            while let Ok(count) = from.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                let allowed = left.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(count as u64))).unwrap_or(0);
                let shown = count.min(allowed as usize);
                let mut out = to();
                let _ = out.write_all(&buffer[..shown]).and_then(|()| out.flush());
                if shown < count {
                    exceeded.store(true, Ordering::SeqCst);
                    break;
                }
            }
        })
    };
    let stdout = copy(Box::new(child.stdout.take().expect("piped")), || Box::new(io::stdout()));
    let stderr = copy(Box::new(child.stderr.take().expect("piped")), || Box::new(io::stderr()));
    let report = loop {
        if exceeded.load(Ordering::SeqCst) {
            break Some(sparrow_core::budget::Exhausted::Output);
        }
        if interrupt.as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            break Some(sparrow_core::budget::Exhausted::Interrupted);
        }
        match child.try_wait() {
            Ok(Some(_)) => break None,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(error) => return Outcome::failure(format!("okkama: java: {error}\n"), EXIT_IO),
        }
    };
    if let Some(exhausted) = report {
        let _ = child.kill();
        let _ = child.wait();
        let _ = stdout.join();
        let _ = stderr.join();
        return Outcome::failure(format!("{}\n", exhausted.message()), 1);
    }
    let status = child.wait();
    let _ = stdout.join();
    let _ = stderr.join();
    // The last bytes may have run past the limit after the program ended.
    if exceeded.load(Ordering::SeqCst) {
        return Outcome::failure(format!("{}\n", sparrow_core::budget::Exhausted::Output.message()), 1);
    }
    let code = status.ok().and_then(|status| status.code()).map_or(1, |code| u8::try_from(code).unwrap_or(1));
    Outcome { stdout: String::new(), stderr: String::new(), code }
}

/// Запуск программы: на Unix процесс заменяется `java` (сигналы и код выхода —
/// у самой программы), иначе — ожидание дочернего процесса.
fn exec_or_wait(command: &mut std::process::Command) -> Outcome {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Outcome::failure(format!("okkama: cannot start java: {error}\n"), EXIT_IO)
    }
    #[cfg(not(unix))]
    {
        match command.status() {
            Ok(status) => Outcome {
                stdout: String::new(),
                stderr: String::new(),
                code: status
                    .code()
                    .map_or(1, |code| u8::try_from(code).unwrap_or(1)),
            },
            Err(error) => {
                Outcome::failure(format!("okkama: cannot start java: {error}\n"), EXIT_IO)
            }
        }
    }
}

/// Классы программы и рантайма в кеше временного каталога (по хешу
/// содержимого: повторный запуск того же кода их не переписывает).
/// Результат — classpath: каталог программы, затем каталог рантайма.
fn cached_classpath(files: &[codegen::ArtifactFile]) -> Result<std::ffi::OsString, String> {
    use std::hash::{Hash, Hasher};
    let root = std::env::temp_dir().join("okkama-run");
    let (runtime, program): (Vec<_>, Vec<_>) = files
        .iter()
        .partition(|file| file.name.starts_with("okkama/rt/"));
    let mut dirs = Vec::new();
    for (prefix, group) in [("program", program), ("runtime", runtime)] {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for file in &group {
            file.name.hash(&mut hasher);
            file.bytes.hash(&mut hasher);
        }
        let dir = root.join(format!("{prefix}-{:016x}", hasher.finish()));
        let marker = dir.join(".complete");
        if !marker.is_file() {
            let staging = root.join(format!("{prefix}-staging-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&staging);
            for file in &group {
                let path = staging.join(&file.name);
                path.parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| std::fs::write(&path, &file.bytes))
                    .map_err(|error| {
                        format!("okkama: cannot write `{}`: {error}\n", path.display())
                    })?;
            }
            let _ = std::fs::write(staging.join(".complete"), b"");
            // Другой запуск мог успеть первым: его каталог равноценен.
            if std::fs::rename(&staging, &dir).is_err() {
                let _ = std::fs::remove_dir_all(&staging);
            }
        }
        dirs.push(dir);
    }
    std::env::join_paths(&dirs).map_err(|error| format!("okkama: invalid class path: {error}\n"))
}

/// Файлы артефакта во временном каталоге; путь каталога или текст ошибки.
fn write_temporary(files: &[codegen::ArtifactFile], prefix: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
    for file in files {
        let path = dir.join(&file.name);
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, &file.bytes));
        if let Err(error) = written {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!(
                "okkama: cannot write `{}`: {error}\n",
                path.display()
            ));
        }
    }
    Ok(dir)
}

/// Команда `java` для класса `main` из classpath `dir` с потоками процесса.
fn java_command(
    dir: impl AsRef<std::ffi::OsStr>,
    main: &str,
    args: &[&str],
    options: &[String],
) -> std::process::Command {
    let mut command = std::process::Command::new("java");
    // Natives на FFM (SQLite, crypto, терминал) без предупреждений о native access.
    command.arg("--enable-native-access=ALL-UNNAMED");
    command
        .args(options)
        .arg("-cp")
        .arg(dir)
        .arg(main)
        .args(args)
        // Байты аргументов как есть: локаль JVM (`LANG=C`) исказила бы UTF-8.
        .env(
            "OKKAMA_ARGV_HEX",
            args.iter()
                .map(|arg| {
                    // `x` перед каждым: пустой аргумент отличим от их отсутствия.
                    let hex: String = arg.bytes().map(|byte| format!("{byte:02x}")).collect();
                    format!("x{hex}")
                })
                .collect::<Vec<_>>()
                .join(","),
        )
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    if std::env::var_os("OKKAMA_ICU_LIBDIR").is_none()
        && let Some(icu) = pinned_icu() {
            command.env("OKKAMA_ICU_LIBDIR", icu);
        }
    command
}

/// Программа вызывает хостовую криптографию: артефакт получает библиотеку
/// `okkama-native` текущей платформы (рядом с `okkama`, или
/// `OKKAMA_NATIVE_LIB`) и библиотеки других платформ из каталогов
/// `OKKAMA_NATIVE_DIR/<os>-<arch>/` — ресурсы `okkama/native/<os>-<arch>/`.
fn with_native_libraries(
    module: &okkama_compiler::ir::Module,
    mut artifact: okkama_compiler::codegen::Artifact,
) -> okkama_compiler::codegen::Artifact {
    let uses = module.functions.iter().any(|function| {
        function
            .extern_symbol()
            .is_some_and(|symbol| symbol.starts_with("runtime/host_crypto@"))
    });
    if !uses {
        return artifact;
    }
    let name = |os: &str| match os {
        "macos" => "libokkama_native.dylib",
        "windows" => "okkama_native.dll",
        _ => "libokkama_native.so",
    };
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let file = name(std::env::consts::OS);
    let local = std::env::var_os("OKKAMA_NATIVE_LIB")
        .map(PathBuf::from)
        .or_else(|| native_library(&platform, file));
    let mut libraries: Vec<(String, PathBuf)> = Vec::new();
    if let Some(path) = local.filter(|path| path.is_file()) {
        libraries.push((platform.clone(), path));
    }
    if let Some(root) = std::env::var_os("OKKAMA_NATIVE_DIR")
        && let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let target = entry.file_name().to_string_lossy().into_owned();
                let os = target.split('-').next().unwrap_or("").to_string();
                let path = entry.path().join(name(&os));
                if path.is_file() && !libraries.iter().any(|(known, _)| *known == target) {
                    libraries.push((target, path));
                }
            }
        }
    for (target, path) in libraries {
        if let Ok(bytes) = std::fs::read(&path) {
            let os = target.split('-').next().unwrap_or("").to_string();
            artifact.files.push(okkama_compiler::codegen::ArtifactFile {
                name: format!("okkama/native/{target}/{}", name(&os)),
                bytes,
            });
        }
    }
    artifact
}

/// `<prefix>/lib/okkama` установки, в которой лежит этот `okkama`
/// (`<prefix>/bin/okkama`); ссылки на исполняемый файл разрешаются.
fn install_root() -> Option<PathBuf> {
    let exe = std::fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    let root = exe.parent()?.parent()?.join("lib").join("okkama");
    root.join("std").is_dir().then_some(root)
}

/// Каталог std: `OKKAMA_STDLIB_DIR`; std установки (`<prefix>/lib/okkama/std`);
/// `std/` репозитория, если `okkama` запущен из его сборки
/// (`okkama-rs/target/release/okkama`); иначе `/usr/local/lib/okkama/std`.
/// Корень std, заданный родителем воркера (`--stdlib`).
static STDLIB_OVERRIDE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn stdlib_dir() -> PathBuf {
    if let Some(dir) = STDLIB_OVERRIDE.get() {
        return dir.clone();
    }
    if let Some(dir) = std::env::var_os("OKKAMA_STDLIB_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(root) = install_root() {
        return root.join("std");
    }
    let exe = std::env::current_exe()
        .ok()
        .and_then(|exe| std::fs::canonicalize(exe).ok());
    if let Some(exe) = exe {
        for ancestor in exe.ancestors().skip(1).take(5) {
            let std = ancestor.join("std");
            if std.join("io").join("stdout.ok").is_file() {
                return std;
            }
        }
    }
    PathBuf::from("/usr/local/lib/okkama/std")
}

/// Библиотека `okkama-native` платформы: рядом с `okkama` (сборка) или в
/// `<prefix>/lib/okkama/native/<os>-<arch>/` (установка).
fn native_library(platform: &str, file: &str) -> Option<PathBuf> {
    let exe = std::fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    let beside = exe.parent()?.join(file);
    if beside.is_file() {
        return Some(beside);
    }
    let installed = install_root()?.join("native").join(platform).join(file);
    installed.is_file().then_some(installed)
}

const ENV_HELP: &str = "okkama env — where this okkama finds its parts

USAGE:
    okkama env

Prints the version, the executable, the std directory, the host crypto
library, the pinned ICU used by locale/collation and the `java` that
`okkama run --engine jvm` starts (the JVM engine needs JDK 22 or newer).
OKKAMA_STDLIB_DIR, OKKAMA_NATIVE_LIB and
OKKAMA_ICU_LIBDIR override the installed locations.
";

fn env_command(args: &[String]) -> Outcome {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Outcome::success(ENV_HELP.to_string());
    }
    if let Some(other) = args.first() {
        return Outcome::usage(&format!("unknown env argument `{other}`"));
    }
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let file = match std::env::consts::OS {
        "macos" => "libokkama_native.dylib",
        "windows" => "okkama_native.dll",
        _ => "libokkama_native.so",
    };
    let found = |path: Option<PathBuf>| match path {
        Some(path) if path.exists() => path.display().to_string(),
        Some(path) => format!("{} (missing)", path.display()),
        None => "(not found)".to_string(),
    };
    let stdlib = stdlib_dir();
    let native = std::env::var_os("OKKAMA_NATIVE_LIB")
        .map(PathBuf::from)
        .or_else(|| native_library(&platform, file));
    let icu = std::env::var_os("OKKAMA_ICU_LIBDIR")
        .map(PathBuf::from)
        .or_else(pinned_icu);
    let java = match java_major() {
        0 => "(not found: `okkama run` needs Java)".to_string(),
        major if major >= 22 => format!("Java {major}"),
        major => format!("Java {major} (too old: the JVM engine needs JDK 22+)"),
    };
    let exe = std::env::current_exe()
        .ok()
        .and_then(|exe| std::fs::canonicalize(exe).ok());
    let text = format!(
        "okkama {}\nplatform  {platform}\nbinary    {}\nstd       {}\nnative    {}\nicu       {}\njava      {java}\n",
        env!("CARGO_PKG_VERSION"),
        found(exe),
        found(Some(stdlib)),
        found(native),
        found(icu),
    );
    Outcome::success(text)
}

/// Закреплённая ICU 76.1 для `runtime/collation`: рядом с std — в
/// репозитории `.runtime/icu-76.1` (`tools/build-icu-collation.sh`), в
/// установке `<prefix>/lib/okkama/icu-76.1`.
fn pinned_icu() -> Option<PathBuf> {
    let root = std::fs::canonicalize(stdlib_dir()).ok()?;
    let parent = root.parent()?;
    [
        parent.join(".runtime/icu-76.1/lib"),
        parent.join("icu-76.1/lib"),
    ]
    .into_iter()
    .find(|dir| dir.is_dir())
}

/// Основная версия `java` из `java -version` (0, если не узнать).
fn java_major() -> u32 {
    let Ok(output) = std::process::Command::new("java").arg("-version").output() else {
        return 0;
    };
    let text = String::from_utf8_lossy(&output.stderr);
    let Some(version) = text.split('"').nth(1) else {
        return 0;
    };
    let mut parts = version.split(['.', '-', '_', '+']);
    match parts.next().and_then(|part| part.parse::<u32>().ok()) {
        Some(1) => parts.next().and_then(|part| part.parse().ok()).unwrap_or(0),
        Some(major) => major,
        None => 0,
    }
}

/// Предупреждения анализа (`file:line:col: warning: …`).
fn render_warnings(source: &SourceFile, warnings: &[lowering::CheckError]) -> String {
    let mut text = String::new();
    for found in warnings {
        let file = found.source.as_ref().unwrap_or(source);
        let location = file.location(found.error.span.start);
        text.push_str(&format!(
            "{}:{}:{}: warning: {}\n",
            file.name(),
            location.line,
            location.column,
            found.error.message
        ));
    }
    text
}

fn ir_error_outcome(source: &SourceFile, span: Span, kind: &str, message: &str) -> Outcome {
    let location = source.location(span.start);
    Outcome::failure(
        format!(
            "{}:{}:{}: {kind}: {message}\n",
            source.name(),
            location.line,
            location.column,
        ),
        EXIT_DIAGNOSTICS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn options_accept_separate_and_inline_values() {
        let line = parse_command_line(
            &args(&["x.ok", "--format", "json", "--mode=production"]),
            CommandOptions::Ast,
        )
        .unwrap();
        assert_eq!(line.input.as_deref(), Some("x.ok"));
        assert_eq!(line.format.as_deref(), Some("json"));
        assert_eq!(line.mode.as_deref(), Some("production"));
    }

    #[test]
    fn ast_only_options_are_rejected_for_tokens() {
        assert!(parse_command_line(&args(&["x.ok", "--spans"]), CommandOptions::Basic).is_err());
        assert!(parse_command_line(&args(&["x.ok", "--spans"]), CommandOptions::Ast).is_ok());
    }

    #[test]
    fn missing_values_and_extra_inputs_are_usage_errors() {
        assert!(parse_command_line(&args(&["x.ok", "--format"]), CommandOptions::Ast).is_err());
        assert!(parse_command_line(&args(&["a.ok", "b.ok"]), CommandOptions::Ast).is_err());
        assert!(parse_command_line(&args(&["--frobnicate"]), CommandOptions::Ast).is_err());
    }

    #[test]
    fn unknown_command_is_a_usage_error() {
        let outcome = run(&args(&["frobnicate", "x.ok"]));
        assert_eq!(outcome.code, EXIT_USAGE);
        assert!(outcome.stdout.is_empty());
    }
}
