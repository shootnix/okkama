//! Native endpoint протокола `ocs-adapter/1` (`conformance/adapters/PROTOCOL.md`).
//!
//! `--ocs-capabilities` reports supported modes for OCS_RUST_ENGINE.
//! `--ocs-request R --ocs-response P` parses/checks the linked source and runs
//! it with JVM (default, as `okkama run`), IR or serialized Sparrow bytecode.
//! Unsupported operations never fall back to another engine. JVM failures
//! are distinguished from application exit codes via OKKAMA_FAILURE_REPORT.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use super::{FileLoader, Outcome, render_all};
use okkama_compiler::codegen::{self, Backend, Options as CodegenOptions};
use okkama_compiler::lowering::{self, LinkError, LoweringError, RootFile};
use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{self, ParseMode, ParseOptions, ast, prune};

const PROTOCOL: &str = "ocs-adapter/1";

pub(super) fn capabilities() -> Outcome {
    let engine = match engine() {
        Ok(engine) => engine,
        Err(message) => return Outcome::failure(format!("{message}\n"), 2),
    };
    // Все движки исполняют программы и тесты.
    let modes = "[\"parse\", \"check\", \"run\", \"test\"]";
    Outcome::success(format!(
        "{{\"protocol\": \"{PROTOCOL}\", \"engine\": \"{engine}\", \"modes\": {modes}}}\n"
    ))
}

fn engine() -> Result<String, String> {
    // Как `okkama run`: JVM по умолчанию, IR и Sparrow — явно.
    let engine = std::env::var("OCS_RUST_ENGINE").unwrap_or_else(|_| "jvm".into());
    match engine.as_str() {
        "ir" | "sparrow" | "jvm" => Ok(engine),
        _ => Err(format!(
            "unknown OCS_RUST_ENGINE `{engine}`; expected ir, sparrow or jvm"
        )),
    }
}

pub(super) fn request(args: &[String]) -> Outcome {
    let mut request_path = None;
    let mut response_path = None;
    let mut index = 0;
    while index + 1 < args.len() {
        match args[index].as_str() {
            "--ocs-request" => request_path = Some(args[index + 1].clone()),
            "--ocs-response" => response_path = Some(args[index + 1].clone()),
            _ => {}
        }
        index += 2;
    }
    let (Some(request_path), Some(response_path)) = (request_path, response_path) else {
        return Outcome::usage(
            "usage: okkama --ocs-request REQUEST.json --ocs-response RESPONSE.json",
        );
    };
    let started = Instant::now();
    let reply = match std::fs::read_to_string(&request_path)
        .map_err(|error| error.to_string())
        .and_then(|text| Json::parse(&text))
    {
        Ok(request) => handle(&request),
        Err(message) => Reply::internal("run", format!("cannot read the request: {message}")),
    };
    let json = reply.to_json(started.elapsed().as_millis());
    if let Err(error) = std::fs::write(&response_path, json) {
        return Outcome::failure(format!("okkama: cannot write the response: {error}\n"), 1);
    }
    Outcome {
        stdout: String::new(),
        stderr: reply.stderr.clone(),
        code: reply.process_exit,
    }
}

/// Наблюдения по одному запросу.
struct Reply {
    mode: String,
    outcome: &'static str,
    stage_reached: &'static str,
    exit_status: Option<u8>,
    failure: Option<Failure>,
    unsupported_reason: Option<String>,
    /// Диагностика, которую CLI напечатал бы в stderr.
    stderr: String,
    process_exit: u8,
}

struct Failure {
    stage: &'static str,
    category: &'static str,
    error_type: Option<String>,
    message: String,
}

impl Reply {
    fn new(mode: &str, outcome: &'static str, stage_reached: &'static str) -> Self {
        Self {
            mode: mode.to_string(),
            outcome,
            stage_reached,
            exit_status: None,
            failure: None,
            unsupported_reason: None,
            stderr: String::new(),
            process_exit: 0,
        }
    }

    fn rejected(mode: &str, stage: &'static str, category: &'static str, text: String) -> Self {
        let mut reply = Self::new(mode, "rejected", stage);
        reply.failure = Some(Failure {
            stage,
            category,
            error_type: None,
            message: text.lines().next().unwrap_or_default().to_string(),
        });
        reply.stderr = text;
        reply.process_exit = 1;
        reply
    }

    fn unsupported(mode: &str, stage: &'static str, reason: String) -> Self {
        let mut reply = Self::new(mode, "unsupported", stage);
        reply.unsupported_reason = Some(reason);
        reply.process_exit = 3;
        reply
    }

    fn internal(mode: &str, message: String) -> Self {
        let mut reply = Self::new(mode, "internal-error", "none");
        reply.failure = Some(Failure {
            stage: "runtime",
            category: "internal",
            error_type: None,
            message,
        });
        reply.process_exit = 70;
        reply
    }

    fn to_json(&self, elapsed_ms: u128) -> String {
        let mut out = String::from("{\n");
        let _ = writeln!(out, "  \"protocol\": {},", quote(PROTOCOL));
        let _ = writeln!(
            out,
            "  \"adapter\": {{\"name\": \"okkama-rs\", \"version\": {}}},",
            quote(env!("CARGO_PKG_VERSION"))
        );
        let _ = writeln!(out, "  \"mode\": {},", quote(&self.mode));
        let _ = writeln!(out, "  \"outcome\": {},", quote(self.outcome));
        let _ = writeln!(out, "  \"stage_reached\": {},", quote(self.stage_reached));
        if let Some(status) = self.exit_status {
            let _ = writeln!(out, "  \"exit_status\": {status},");
        }
        if let Some(reason) = &self.unsupported_reason {
            let _ = writeln!(out, "  \"unsupported_reason\": {},", quote(reason));
        }
        if let Some(failure) = &self.failure {
            let _ = write!(
                out,
                "  \"failure\": {{\"stage\": {}, \"category\": {}, \"message\": {}, \"raw\": {}",
                quote(failure.stage),
                quote(failure.category),
                quote(&failure.message),
                quote(&self.stderr)
            );
            if let Some(error_type) = &failure.error_type {
                let _ = write!(out, ", \"error_type\": {}", quote(error_type));
            }
            out.push_str("},\n");
        }
        let _ = writeln!(out, "  \"elapsed_ms\": {elapsed_ms}");
        out.push_str("}\n");
        out
    }
}

fn handle(request: &Json) -> Reply {
    let mode = request
        .get("mode")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_string();
    if !matches!(mode.as_str(), "parse" | "check" | "run" | "test") {
        return Reply::unsupported(&mode, "none", format!("mode `{mode}` is not implemented"));
    }
    let sources: Vec<&str> = request
        .get("sources")
        .and_then(Json::as_array)
        .map(|items| items.iter().filter_map(Json::as_str).collect())
        .unwrap_or_default();
    if sources.is_empty() {
        return Reply::internal(&mode, "the request names no sources".to_string());
    }
    // Программы исполняются в production-грамматике: тела `test` не разбираются (§14);
    // тестовый режим разбирает их и исполняет тесты корня.
    let testing = mode == "test";
    let options = ParseOptions {
        mode: if testing {
            ParseMode::Full
        } else {
            ParseMode::Production
        },
        ..ParseOptions::default()
    };
    // Несколько файлов — один модуль (§5.1): каждый разбирается отдельно.
    let mut files = Vec::new();
    for path in &sources {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => return Reply::internal(&mode, format!("cannot read `{path}`: {error}")),
        };
        let source = match SourceFile::new(*path, text) {
            Ok(source) => source,
            Err(error) => {
                return Reply::rejected(&mode, "parse", "syntax", format!("{path}: {error}\n"));
            }
        };
        let program = match syntax::parse(&source, &options) {
            Ok(program) => program,
            Err(diagnostics) => {
                return Reply::rejected(
                    &mode,
                    "parse",
                    "syntax",
                    render_all(&diagnostics, &source),
                );
            }
        };
        files.push((source, program));
    }
    if mode == "parse" {
        return Reply::new(&mode, "accepted", "parse");
    }
    let files: Vec<(SourceFile, ast::Program)> = files
        .into_iter()
        .map(|(source, program)| {
            let program = if testing {
                prune::desugar_test_items(program)
            } else {
                prune::strip_test_items(program)
            };
            (source, program)
        })
        .collect();
    let roots: Vec<RootFile<'_>> = files
        .iter()
        .map(|(source, program)| RootFile { program, source })
        .collect();
    let source = &files[0].0;
    let mut loader = match request.get("stdlib_root").and_then(Json::as_str) {
        Some(root) => FileLoader::with_stdlib(PathBuf::from(root)),
        None => FileLoader::from_env(),
    };
    if testing {
        loader = loader.for_tests();
    }
    // Семантический анализ — авторитет для ошибок программы; lowering после
    // него сообщает только ограничения backend (и пока — проверки потока).
    // Предупреждения в conformance не печатаются: stderr сравнивается.
    let checked = if testing {
        lowering::check_linked_tests(&roots, &mut loader)
    } else {
        lowering::check_linked_files(&roots, &mut loader)
    };
    match checked {
        Ok(_) => {}
        Err(Err(text)) => return Reply::rejected(&mode, "parse", "syntax", text),
        Err(Ok(errors)) => {
            let first = errors
                .iter()
                .find(|found| !found.error.unsupported)
                .unwrap_or(&errors[0]);
            let file = first.source.as_ref().unwrap_or(source);
            let location = file.location(first.error.span.start);
            let kind = if first.error.unsupported {
                "analysis limitation"
            } else {
                "semantic error"
            };
            let text = format!(
                "{}:{}:{}: {kind}: {}\n",
                file.name(),
                location.line,
                location.column,
                first.error.message
            );
            if first.error.unsupported {
                return Reply::unsupported(&mode, "check", text.trim_end().to_string());
            }
            let category = if first.error.message.starts_with("Import ") {
                "resolution"
            } else {
                "semantic"
            };
            return Reply::rejected(&mode, "check", category, text);
        }
    }
    let mut loader = match request.get("stdlib_root").and_then(Json::as_str) {
        Some(root) => FileLoader::with_stdlib(PathBuf::from(root)),
        None => FileLoader::from_env(),
    };
    if testing {
        loader = loader.for_tests();
    }
    let lowered = if testing {
        lowering::lower_linked_tests(&roots, &mut loader)
    } else {
        lowering::lower_linked_files(&roots, &mut loader)
    };
    let module = match lowered {
        Ok((module, _)) => module,
        Err(LinkError::Load(text)) => return Reply::rejected(&mode, "parse", "syntax", text),
        Err(LinkError::Lowering {
            source: imported,
            error,
        }) => {
            let file = imported.as_ref().unwrap_or(source);
            let location = file.location(error.span().start);
            let kind = match error {
                LoweringError::Unsupported { .. } => "lowering error",
                LoweringError::Invalid { .. } => "semantic error",
            };
            let text = format!(
                "{}:{}:{}: {kind}: {}\n",
                file.name(),
                location.line,
                location.column,
                error.message()
            );
            return match error {
                // Программа прошла анализ: ограничение lowering — не её ошибка.
                LoweringError::Unsupported { .. } if mode == "check" => {
                    Reply::new(&mode, "accepted", "check")
                }
                LoweringError::Unsupported { .. } => {
                    Reply::unsupported(&mode, "check", text.trim_end().to_string())
                }
                LoweringError::Invalid { .. } => {
                    let category = if error.message().starts_with("Import ") {
                        "resolution"
                    } else {
                        "semantic"
                    };
                    Reply::rejected(&mode, "check", category, text)
                }
            };
        }
    };
    if mode == "check" {
        return Reply::new(&mode, "accepted", "check");
    }
    // Исполняемой программе нужна точка входа (текст Perl-интерпретатора).
    if module.entry.is_none() && !testing {
        return Reply::rejected(
            &mode,
            "check",
            "semantic",
            format!(
                "{}: semantic error: Program requires main() declaration\n",
                source.name()
            ),
        );
    }
    run(&mode, &module, request)
}

/// Собрать классы во временный каталог и запустить `java`.
fn run(mode: &str, module: &okkama_compiler::ir::Module, request: &Json) -> Reply {
    let engine = match engine() {
        Ok(engine) => engine,
        Err(message) => return Reply::internal(mode, message),
    };
    if engine != "jvm" {
        return run_native(mode, module, request, &engine);
    }
    let options = CodegenOptions {
        artifact_name: "Main".to_string(),
    };
    let artifact = match codegen::generate(module, Backend::Jvm, &options) {
        Ok(artifact) => super::with_native_libraries(module, artifact),
        Err(error) => return Reply::unsupported(mode, "check", format!("codegen error: {error}")),
    };
    let dir = match super::write_temporary(&artifact.files, "okkama-ocs") {
        Ok(dir) => dir,
        Err(text) => return Reply::internal(mode, text.trim_end().to_string()),
    };
    let report = dir.join("failure.report");
    let args: Vec<&str> = request
        .get("args")
        .and_then(Json::as_array)
        .map(|items| items.iter().filter_map(Json::as_str).collect())
        .unwrap_or_default();
    let mut command = super::java_command(&dir, "Main", &args);
    command.env("OKKAMA_FAILURE_REPORT", &report);
    if let Some(cwd) = request.get("cwd").and_then(Json::as_str) {
        command.current_dir(cwd);
    }
    let status = command.status();
    let recorded = std::fs::read_to_string(&report).ok();
    let _ = std::fs::remove_dir_all(&dir);
    let status = match status {
        Ok(status) => status,
        Err(error) => return Reply::internal(mode, format!("cannot start java: {error}")),
    };
    let Some(code) = status.code() else {
        return Reply::internal(mode, format!("java terminated by a signal: {status}"));
    };
    let code = u8::try_from(code).unwrap_or(1);
    match recorded {
        Some(record) => {
            let mut parts = record.splitn(3, '\t');
            let kind = parts.next().unwrap_or("runtime");
            let error_type = parts.next().unwrap_or("").to_string();
            let message = parts.next().unwrap_or("").to_string();
            let mut reply = Reply::new(mode, "failed", "runtime");
            reply.failure = Some(Failure {
                stage: "runtime",
                category: match kind {
                    "panic" => "panic",
                    "uncaught-error" => "uncaught-error",
                    _ => "runtime",
                },
                error_type: (!error_type.is_empty()).then_some(error_type),
                message,
            });
            reply.process_exit = code.max(1);
            reply
        }
        None => {
            let mut reply = Reply::new(mode, "completed", "runtime");
            reply.exit_status = Some(code);
            reply.process_exit = code;
            reply
        }
    }
}

/// Execute native IR or serialized Sparrow bytecode, without a JVM fallback.
fn run_native(
    mode: &str,
    module: &okkama_compiler::ir::Module,
    request: &Json,
    engine: &str,
) -> Reply {
    use okkama_runtime::RuntimeError;
    let args: Vec<String> = request
        .get("args")
        .and_then(Json::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    {
        let exe = std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "okkama".into());
        let mut command = vec![exe, "run".into()];
        if let Some(root) = request.get("stdlib_root").and_then(Json::as_str) {
            command.push("--stdlib".into());
            command.push(root.to_string());
        }
        command.push("--engine".into());
        command.push(engine.to_string());
        if mode == "test" {
            command.push("-t".into());
        }
        for source in request
            .get("sources")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
        {
            let absolute = std::fs::canonicalize(source).unwrap_or_else(|_| source.into());
            command.push(absolute.display().to_string());
        }
        okkama_runtime::set_worker_command(command);
    }
    if let Some(cwd) = request.get("cwd").and_then(Json::as_str) {
        if let Err(error) = std::env::set_current_dir(cwd) {
            return Reply::internal(mode, format!("cannot enter `{cwd}`: {error}"));
        }
    }
    let result = if engine == "ir" {
        if mode == "test" {
            // Отчёт тестов — в stdout процесса; код выхода 0 — все прошли.
            let mut stdout = std::io::stdout();
            okkama_runtime::run_tests(module, &mut stdout)
                .map(|passed| okkama_compiler::ir::IntValue::from_i64(i64::from(!passed)))
        } else {
            okkama_runtime::execute_with_arguments(module, args)
        }
    } else {
        let program = match codegen::sparrow::generate(module) {
            Ok(program) => program,
            Err(error) => return Reply::unsupported(mode, "check", error.to_string()),
        };
        // Exercise the file format as well as codegen and VM on every OCS case.
        let loaded = match okkama_sparrow::encode(&program).and_then(|bytes| okkama_sparrow::decode(&bytes)) {
            Ok(program) => program,
            Err(error) => return Reply::internal(mode, error.to_string()),
        };
        let outcome = if mode == "test" {
            let mut stdout = std::io::stdout();
            okkama_runtime::sparrow::run_tests(&loaded, &mut stdout)
                .map(|passed| okkama_compiler::ir::IntValue::from_i64(i64::from(!passed)))
        } else {
            okkama_runtime::sparrow::execute_with_arguments(&loaded, args)
        };
        match outcome {
            Ok(value) => Ok(value),
            Err(okkama_runtime::sparrow::Error::Operation { error, .. }) => Err(error),
            Err(error) => return Reply::internal(mode, error.to_string()),
        }
    };
    let message = match result {
        Ok(value) => match value.to_i64().and_then(|value| u8::try_from(value).ok()) {
            Some(code) => {
                let mut reply = Reply::new(mode, "completed", "runtime");
                reply.exit_status = Some(code);
                reply.process_exit = code;
                return reply;
            }
            None => "main result must be in 0..255".to_string(),
        },
        Err(RuntimeError::Unsupported { message, .. }) => {
            return Reply::unsupported(mode, "runtime", message);
        }
        Err(RuntimeError::Unwind { failure, .. }) => {
            let mut reply = Reply::new(mode, "failed", "runtime");
            reply.stderr = format!("{failure}\n");
            reply.failure = Some(Failure {
                stage: "runtime",
                category: failure.category(),
                error_type: match &failure.kind {
                    okkama_runtime::FailureKind::Thrown { name, .. } => Some(name.clone()),
                    okkama_runtime::FailureKind::Native(_) => Some("NativeFailure".into()),
                    _ => None,
                },
                message: failure.message().into(),
            });
            reply.process_exit = 1;
            return reply;
        }
        Err(RuntimeError::Panic { message, .. }) => {
            let mut reply = Reply::new(mode, "failed", "runtime");
            reply.stderr = format!("Panic: {message}\n");
            reply.failure = Some(Failure {
                stage: "runtime",
                category: "panic",
                error_type: None,
                message,
            });
            reply.process_exit = 1;
            return reply;
        }
        Err(RuntimeError::Trap { message, .. }) => message,
        Err(RuntimeError::NativeFailure { failure, .. }) => {
            let mut reply = Reply::new(mode, "failed", "runtime");
            reply.stderr = format!("{failure}\n");
            reply.failure = Some(Failure {
                stage: "runtime",
                category: "runtime",
                error_type: Some("NativeFailure".into()),
                message: failure.message,
            });
            reply.process_exit = 1;
            return reply;
        }
        Err(error) => return Reply::internal(mode, error.to_string()),
    };
    let mut reply = Reply::new(mode, "failed", "runtime");
    reply.stderr = format!("RuntimeError: {message}\n");
    reply.failure = Some(Failure {
        stage: "runtime",
        category: "runtime",
        error_type: None,
        message,
    });
    reply.process_exit = 1;
    reply
}

fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Минимальный JSON для запросов адаптера (без внешних зависимостей).
enum Json {
    /// `null`, `true`, `false` и числа: запросу адаптера их значения не нужны.
    Scalar,
    Str(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn parse(text: &str) -> Result<Json, String> {
        let mut parser = JsonParser {
            chars: text.chars().collect(),
            at: 0,
        };
        let value = parser.value()?;
        parser.space();
        if parser.at != parser.chars.len() {
            return Err("trailing characters after JSON".to_string());
        }
        Ok(value)
    }

    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(map) => map.get(key),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }
}

struct JsonParser {
    chars: Vec<char>,
    at: usize,
}

impl JsonParser {
    fn space(&mut self) {
        while self.chars.get(self.at).is_some_and(|ch| ch.is_whitespace()) {
            self.at += 1;
        }
    }

    fn eat(&mut self, expected: char) -> Result<(), String> {
        self.space();
        if self.chars.get(self.at) == Some(&expected) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected `{expected}` at {}", self.at))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.chars.get(self.at) {
            Some('{') => {
                self.at += 1;
                let mut map = BTreeMap::new();
                self.space();
                if self.chars.get(self.at) == Some(&'}') {
                    self.at += 1;
                    return Ok(Json::Object(map));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    self.eat(':')?;
                    let value = self.value()?;
                    map.insert(key, value);
                    self.space();
                    match self.chars.get(self.at) {
                        Some(',') => self.at += 1,
                        Some('}') => {
                            self.at += 1;
                            return Ok(Json::Object(map));
                        }
                        _ => return Err(format!("bad object at {}", self.at)),
                    }
                }
            }
            Some('[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.chars.get(self.at) == Some(&']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.space();
                    match self.chars.get(self.at) {
                        Some(',') => self.at += 1,
                        Some(']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(format!("bad array at {}", self.at)),
                    }
                }
            }
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.word("true", Json::Scalar),
            Some('f') => self.word("false", Json::Scalar),
            Some('n') => self.word("null", Json::Scalar),
            Some(_) => {
                let start = self.at;
                while self.chars.get(self.at).is_some_and(|ch| {
                    ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E')
                }) {
                    self.at += 1;
                }
                let text: String = self.chars[start..self.at].iter().collect();
                text.parse::<f64>()
                    .map(|_| Json::Scalar)
                    .map_err(|_| format!("bad number at {start}"))
            }
            None => Err("unexpected end of JSON".to_string()),
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        let end = self.at + word.len();
        if end <= self.chars.len() && self.chars[self.at..end].iter().collect::<String>() == word {
            self.at = end;
            Ok(value)
        } else {
            Err(format!("bad literal at {}", self.at))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.chars.get(self.at) != Some(&'"') {
            return Err(format!("expected a string at {}", self.at));
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(ch) = self.chars.get(self.at).copied() else {
                return Err("unterminated string".to_string());
            };
            self.at += 1;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let Some(escape) = self.chars.get(self.at).copied() else {
                        return Err("unterminated escape".to_string());
                    };
                    self.at += 1;
                    match escape {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'u' => {
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code)
                                && self.chars.get(self.at) == Some(&'\\')
                                && self.chars.get(self.at + 1) == Some(&'u')
                            {
                                self.at += 2;
                                let low = self.hex4()?;
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        other => out.push(other),
                    }
                }
                ch => out.push(ch),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let end = self.at + 4;
        if end > self.chars.len() {
            return Err("short \\u escape".to_string());
        }
        let text: String = self.chars[self.at..end].iter().collect();
        self.at = end;
        u32::from_str_radix(&text, 16).map_err(|_| "bad \\u escape".to_string())
    }
}
