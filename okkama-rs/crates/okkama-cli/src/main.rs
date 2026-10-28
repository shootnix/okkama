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
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod lsp;
mod ocs;

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
    okkama lsp
    okkama env
    okkama ir <FILE|-> [--format text]
    okkama build <FILE|-> --target jvm -o <FILE>
    okkama build <FILE|-> --target sparrow -o <FILE.spb>
    okkama exec <FILE.spb>
    okkama run [-t] <FILE> [FILE.ok...] [ARGS...]
    okkama run --engine ir|sparrow <FILE> [FILE.ok...]
    okkama <FILE.ok> [FILE.ok...]
    okkama help [COMMAND]
    okkama --version

COMMANDS:
    tokens    Print the token stream of a source file
    ast       Parse a source file and print its syntax tree
    check     Check a program and its imports; report every semantic error
    fmt       Format source files (`-w` rewrites them, `--check` only reports)
    lint      Check programs and report errors, warnings and lint findings
    env       Show where okkama finds std, the native library, ICU and Java
    lsp       Run the language server (LSP over stdio) for editors
    ir        Lower a source module to IR and print its functions
    build     Lower a source module and invoke the selected backend
    exec      Execute a Sparrow bytecode file in this process
    run       Run a program: the IR interpreter by default, `--engine sparrow`
              (VM with JIT) or `--engine jvm` (build classes and run `java`)

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
    --target jvm          Select the backend (required; Java 8+ class files)
    --target sparrow      Select Sparrow bytecode (.spb)
    --backend NAME        Compatibility alias for --target
    -o, --output FILE      Output artifact path (required)

RUN OPTIONS:
    --engine ir           Execute IR inside the Rust process (default)
    --engine sparrow      Compile to Sparrow and execute it (JIT) in the process
    --engine jvm          Compile to JVM classes and execute them with `java`
                          Place engine options before FILE; every engine runs
                          tests (-t), program arguments and `spawn` workers.
                          OKKAMA_ENGINE=ir|sparrow|jvm sets the default.

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
    okkama ir <FILE|-> [--format text]

OPTIONS:
    --format text     Output format (default: text)

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
    okkama build <FILE|-> --target jvm -o <FILE>
    okkama build <FILE|-> --target sparrow -o <FILE.spb>

OPTIONS:
    --target jvm          Select the backend (required)
    --target sparrow      Generate Sparrow bytecode
    --backend NAME        Compatibility alias for --target
    -o, --output FILE      Output artifact path (required)

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
    okkama exec <FILE.spb> [ARGS...]

Loads and verifies an SPB file, then executes it in the Rust process with
ARGS as the program's arguments; a test-profile file (`build -t`) runs its
tests. Source files and Java are not required. main must return an integer
in 0..255, used as the process exit status; an uncaught failure is reported
on stderr with exit 1. File-format errors exit with 1, usage errors with 2,
and read errors with 3.
";

fn sparrow_exec_command(args: &[String]) -> Outcome {
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
    let exe = std::env::current_exe().map_or_else(|_| "okkama".to_string(), |exe| exe.display().to_string());
    let file = std::fs::canonicalize(path).map_or_else(|_| path.clone(), |file| file.display().to_string());
    okkama_runtime::set_worker_command(vec![exe, "exec".into(), file]);
    if program.tests.is_some() {
        let mut stdout = io::stdout();
        return match okkama_runtime::sparrow::run_tests(&program, &mut stdout) {
            Ok(passed) => Outcome {
                stdout: String::new(),
                stderr: String::new(),
                code: u8::from(!passed),
            },
            Err(error) => Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
        };
    }
    sparrow_outcome(okkama_runtime::sparrow::execute_with_arguments(&program, arguments.to_vec()))
}

fn sparrow_outcome(
    result: Result<ir::IntValue, okkama_runtime::sparrow::Error>,
) -> Outcome {
    match result {
        Ok(value) => native_outcome(Ok(value)),
        Err(okkama_runtime::sparrow::Error::Operation { error, .. }) => native_outcome(Err(error)),
        Err(error) => Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
    }
}

/// Исход программы IR/Sparrow — как у JVM-рантайма (`Rt.exit`, `Rt.report`):
/// код `main`, а отказ — диагностика в stderr и код 1. Ограничение
/// реализации — диагностика `okkama:`.
fn native_outcome(result: Result<ir::IntValue, okkama_runtime::RuntimeError>) -> Outcome {
    match result {
        Ok(value) => match value.to_i64().and_then(|code| u8::try_from(code).ok()) {
            Some(code) => Outcome {
                stdout: String::new(),
                stderr: String::new(),
                code,
            },
            None => Outcome::failure(
                format!("RuntimeError: main() must return an int in 0..255, got {value}\n"),
                1,
            ),
        },
        Err(error) => match okkama_runtime::report(&error) {
            Some(text) => Outcome::failure(text, 1),
            None => Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
        },
    }
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Сервер языка владеет stdin/stdout сам.
    if args.first().map(String::as_str) == Some("lsp") {
        return lsp::run();
    }
    let outcome = run(&args);
    // A closed stdout (`okkama ast big.ok | head`) is not an error worth a
    // panic; anything printed so far is all the reader wanted.
    let _ = io::stdout().lock().write_all(outcome.stdout.as_bytes());
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
        "ir" => ir_command(rest),
        "build" => build_command(rest),
        "exec" => sparrow_exec_command(rest),
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
unused-private, unreachable, snake-case. Prints
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
            if formatted != text {
                if let Err(error) = std::fs::write(file, &formatted) {
                    stderr.push_str(&format!("okkama: cannot write `{file}`: {error}\n"));
                    io_failed = true;
                }
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

/// Импорты из файлов, как в Perl-интерпретаторе: каталоги `OKKAMA_IMPORT_DIRS`
/// (через `:`), каталог импортирующего файла, затем std
/// (`OKKAMA_STDLIB_DIR`, по умолчанию `/usr/local/lib/okkama/std`).
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
        let file = if path.ends_with(".ok") {
            path.to_string()
        } else {
            format!("{path}.ok")
        };
        let base = if importer == "<stdin>" {
            PathBuf::from(".")
        } else {
            Path::new(importer)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        };
        let candidates = self
            .import_dirs
            .iter()
            .chain([&base, &self.stdlib])
            .map(|dir| dir.join(&file));
        for candidate in candidates {
            if !candidate.is_file() {
                continue;
            }
            let canonical = std::fs::canonicalize(&candidate).unwrap_or(candidate);
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
    let options = CodegenOptions {
        artifact_name: artifact_name.to_string(),
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
        if let Some(parent) = path.parent().filter(|_| index > 0) {
            if let Err(error) = std::fs::create_dir_all(parent) {
                return Outcome::failure(
                    format!("okkama: cannot create `{}`: {error}\n", parent.display()),
                    EXIT_IO,
                );
            }
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

/// `okkama run`: по умолчанию IR-интерпретатор в процессе (все три движка
/// исполняют язык и std одинаково; `OKKAMA_ENGINE` меняет умолчание);
/// --engine jvm собирает классы и запускает java с потоками и кодом выхода
/// программы. Файлы модуля — первый и следующие
/// за ним существующие `.ok`; остальное (после необязательного `--`) —
/// аргументы программы.
fn run_command(args: &[String]) -> Outcome {
    let mut rest = args;
    let mut tests = false;
    let default = std::env::var("OKKAMA_ENGINE").unwrap_or_default();
    let mut engine = match default.as_str() {
        "ir" => "ir",
        "sparrow" => "sparrow",
        "jvm" => "jvm",
        _ => "ir",
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
                    return Outcome::usage("--engine needs a value: ir, sparrow or jvm");
                };
                if !matches!(value.as_str(), "ir" | "sparrow" | "jvm") {
                    return Outcome::usage("unknown engine (use ir, sparrow or jvm)");
                }
                engine = value;
                rest = &rest[1..];
            }
            "-t" | "--tests" => tests = true,
            "-h" | "--help" => return Outcome::success(HELP.to_string()),
            "--" => {
                rest = &rest[1..];
                break;
            }
            other if other.starts_with('-') && other != "-" => {
                return Outcome::usage(&format!("unknown option `{other}`"));
            }
            _ => break,
        }
        rest = &rest[1..];
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
    let (module, warnings) = match lower_source(&sources, line.tests) {
        Ok(lowered) => lowered,
        Err(outcome) => return outcome,
    };
    let _ = io::stderr().lock().write_all(warnings.as_bytes());
    if engine != "jvm" && std::env::var_os("OKKAMA_ICU_LIBDIR").is_none() {
        // Как у JVM-запуска: программа видит закреплённую ICU в окружении.
        if let Some(icu) = pinned_icu() {
            okkama_runtime::default_environment("OKKAMA_ICU_LIBDIR", &icu.display().to_string());
        }
    }
    if engine == "ir" {
        okkama_runtime::set_worker_command(worker_command("ir", tests, &line));
        if tests {
            let mut stdout = io::stdout();
            return match okkama_runtime::run_tests(&module, &mut stdout) {
                Ok(passed) => Outcome {
                    stdout: String::new(),
                    stderr: String::new(),
                    code: u8::from(!passed),
                },
                Err(error) => Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
            };
        }
        let arguments = program_args.to_vec();
        return native_outcome(okkama_runtime::execute_with_arguments(&module, arguments));
    }
    if engine == "sparrow" {
        let program = match codegen::sparrow::generate(&module) {
            Ok(program) => program,
            Err(error) => return Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
        };
        okkama_runtime::set_worker_command(worker_command("sparrow", tests, &line));
        if tests {
            let mut stdout = io::stdout();
            return match okkama_runtime::sparrow::run_tests(&program, &mut stdout) {
                Ok(passed) => Outcome {
                    stdout: String::new(),
                    stderr: String::new(),
                    code: u8::from(!passed),
                },
                Err(error) => Outcome::failure(format!("okkama: {error}\n"), EXIT_DIAGNOSTICS),
            };
        }
        return sparrow_outcome(okkama_runtime::sparrow::execute_with_arguments(&program, program_args.to_vec()));
    }
    let options = CodegenOptions {
        artifact_name: "Main".to_string(),
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
    let args: Vec<&str> = program_args.iter().map(String::as_str).collect();
    let mut command = java_command(&classpath, "Main", &args);
    exec_or_wait(&mut command)
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
) -> std::process::Command {
    let mut command = std::process::Command::new("java");
    // JDK 22+: natives на FFM (SQLite) без предупреждений о native access.
    if java_major() >= 22 {
        command.arg("--enable-native-access=ALL-UNNAMED");
    }
    command
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
    if std::env::var_os("OKKAMA_ICU_LIBDIR").is_none() {
        if let Some(icu) = pinned_icu() {
            command.env("OKKAMA_ICU_LIBDIR", icu);
        }
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
    if let Some(root) = std::env::var_os("OKKAMA_NATIVE_DIR") {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let target = entry.file_name().to_string_lossy().into_owned();
                let os = target.split('-').next().unwrap_or("").to_string();
                let path = entry.path().join(name(&os));
                if path.is_file() && !libraries.iter().any(|(known, _)| *known == target) {
                    libraries.push((target, path));
                }
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
`okkama run` starts (JDK 22 or newer enables the FFM natives: crypto, SQLite,
terminal, accounts, collation). OKKAMA_STDLIB_DIR, OKKAMA_NATIVE_LIB and
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
        major if major >= 22 => format!("Java {major} (FFM natives enabled)"),
        major => format!("Java {major} (JDK 22+ needed for crypto, SQLite and other FFM natives)"),
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

/// Команда процесса-воркера `spawn`: та же программа тем же движком.
pub(crate) fn worker_command(engine: &str, tests: bool, line: &CommandLine) -> Vec<String> {
    let exe = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "okkama".into());
    let stdlib = std::fs::canonicalize(stdlib_dir()).unwrap_or_else(|_| stdlib_dir());
    let mut command = vec![
        exe,
        "run".into(),
        "--stdlib".into(),
        stdlib.display().to_string(),
        "--engine".into(),
        engine.into(),
    ];
    if tests {
        command.push("-t".into());
    }
    for input in line.input.iter().chain(&line.more_inputs) {
        let absolute = std::fs::canonicalize(input).unwrap_or_else(|_| PathBuf::from(input));
        command.push(absolute.display().to_string());
    }
    command
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
