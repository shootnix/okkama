//! `okkama repl`: интерактивные проверки на IR-интерпретаторе.
//!
//! Ввод — объявления (`object`, `enum`, `role`, `native`, `cap`), импорт или
//! инструкции. Объявления и импорты копятся в файле `<repl>`. Инструкции
//! становятся ячейкой — вторым файлом того же модуля (§5.1) с действием
//! `ReplCellN.run(<прошлые переменные>)`. Исполняется только новая ячейка: её
//! переменные возвращаются через `ReplKeep<T>` и переходят в следующую, а
//! побочные эффекты прошлых ячеек не повторяются. Значение последнего
//! выражения печатает `debug/dumper`.
use super::{lower_source, pinned_icu};
use okkama_compiler::ir;
use okkama_compiler::source::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions, ast};
use okkama_runtime::repl::{Carried, run_cell};
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

/// Служебный generic-объект: имя экземпляра — тип переменной.
const KEEPER: &str = "ReplKeep";
/// Файл ввода: первая строка — служебная, ввод начинается со второй.
const INPUT: &str = "<input>";

const HELP: &str = "\
Enter statements, expressions, declarations (object, enum, role) or imports.
The value of a trailing expression is dumped; variables live on between inputs.

    :vars     variables and their types
    :decls    declarations and imports so far
    :reset    forget everything
    :help     this text
    :quit     leave (also Ctrl-D)

Actions (closures) and futures are not kept between inputs.
";

struct Var {
    name: String,
    ty: String,
    value: Carried,
}

#[derive(Default)]
struct Session {
    imports: Vec<String>,
    decls: Vec<String>,
    vars: Vec<Var>,
    cells: usize,
}

pub(crate) fn run() -> ExitCode {
    if std::env::var_os("OKKAMA_ICU_LIBDIR").is_none()
        && let Some(icu) = pinned_icu()
    {
        okkama_runtime::default_environment("OKKAMA_ICU_LIBDIR", &icu.display().to_string());
    }
    let interactive = io::stdin().is_terminal();
    if interactive {
        println!("okkama {} REPL (IR engine) — :help for commands", env!("CARGO_PKG_VERSION"));
    }
    let mut session = Session::default();
    let mut input = String::new();
    let stdin = io::stdin();
    loop {
        if interactive {
            print!("{}", if input.is_empty() { "okkama> " } else { "   ...> " });
            let _ = io::stdout().flush();
        }
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if input.is_empty() && line.trim().is_empty() {
            continue;
        }
        if input.is_empty() && line.trim_start().starts_with(':') {
            if !session.command(line.trim()) {
                return ExitCode::SUCCESS;
            }
            continue;
        }
        input.push_str(&line);
        if complete(&input) {
            session.eval(std::mem::take(&mut input).trim());
        }
    }
    if !input.trim().is_empty() {
        session.eval(input.trim());
    }
    ExitCode::SUCCESS
}

impl Session {
    /// Команда `:...`; `false` — выйти.
    fn command(&mut self, command: &str) -> bool {
        match command {
            ":quit" | ":q" | ":exit" => return false,
            ":help" | ":h" => print!("{HELP}"),
            ":vars" => {
                if self.vars.is_empty() {
                    println!("(no variables)");
                }
                for var in &self.vars {
                    println!("{}: {}", var.name, var.ty);
                }
            }
            ":decls" => {
                for import in &self.imports {
                    println!("{import}");
                }
                for decl in &self.decls {
                    println!("{decl}");
                }
            }
            ":reset" => *self = Self::default(),
            other => eprintln!("unknown command `{other}` (:help lists commands)"),
        }
        let _ = io::stdout().flush();
        true
    }

    fn eval(&mut self, text: &str) {
        let first = text.split_whitespace().next().unwrap_or("");
        match first {
            "import" => {
                self.imports.push(text.to_string());
                if let Err(errors) = self.compile(None) {
                    self.imports.pop();
                    eprint!("{errors}");
                }
            }
            "object" | "enum" | "role" | "native" | "cap" => {
                if let Err(errors) = self.compile(Some(&format!("{}\n{text}\n", self.header()))) {
                    eprint!("{errors}");
                } else {
                    self.decls.push(text.to_string());
                }
            }
            _ => self.statements(text),
        }
        let _ = io::stdout().flush();
    }

    /// Первая строка файла ввода: модуль и импорты (они у каждого файла свои).
    fn header(&self) -> String {
        let mut header = String::from("module repl v1.0.0;");
        for import in &self.imports {
            header.push(' ');
            header.push_str(import);
        }
        header
    }

    /// Файл объявлений `<repl>`, а с `input` — и файл ввода того же модуля.
    fn compile(&self, input: Option<&str>) -> Result<ir::Module, String> {
        let mut decls = String::from(
            "module repl v1.0.0;\nimport \"debug/dumper\" as repl_dumper;\nimport \"io/stdout\" as repl_io;\n",
        );
        for import in &self.imports {
            decls.push_str(import);
            decls.push('\n');
        }
        for decl in &self.decls {
            decls.push_str(decl);
            decls.push('\n');
        }
        decls.push_str(&format!(
            "object {KEEPER}<T> {{ has value is T; {KEEPER}(:value); }}\n\
             object ReplShow {{ ReplShow(); show(value is any) is void \
             {{ repl_io.Stdout().writeln(repl_dumper.Dumper(value).all()); }} }}\n\
             main() is int {{ 0; }}\n"
        ));
        let mut sources = vec![SourceFile::new("<repl>", decls).map_err(|error| format!("{error}\n"))?];
        if let Some(input) = input {
            sources.push(SourceFile::new(INPUT, input).map_err(|error| format!("{error}\n"))?);
        }
        lower_source(&sources, false)
            .map(|(module, _)| module)
            .map_err(|outcome| shift_input_lines(&outcome.stderr))
    }

    fn statements(&mut self, text: &str) {
        // `;` в конце можно не писать: дописывается, если так ввод разбирается
        // (после `if { }` она лишняя, после `xs.map() -> v { }` — нужна).
        let closed = format!("{text};");
        let (body, shape) = match Shape::of(&closed) {
            Some(found) if !text.ends_with(';') => (closed, found),
            _ => (text.to_string(), Shape::of(text).unwrap_or_default()),
        };
        // Прошлая переменная, объявленная заново, — новая переменная ячейки.
        let carried: Vec<usize> = (0..self.vars.len())
            .filter(|index| !shape.lets.contains(&self.vars[*index].name))
            .collect();
        let mut kept: Vec<String> = carried.iter().map(|index| self.vars[*index].name.clone()).collect();
        for name in &shape.lets {
            if !kept.contains(name) {
                kept.push(name.clone());
            }
        }
        self.cells += 1;
        let cell = format!("ReplCell{}", self.cells);
        let params: Vec<String> = carried
            .iter()
            .map(|index| format!("{} is {}", self.vars[*index].name, self.vars[*index].ty))
            .collect();
        let source = |body: &str| {
            let keeps: Vec<String> = kept.iter().map(|name| format!("{KEEPER}(value: {name})")).collect();
            format!(
                "{} object {cell} {{ {cell}(); run({}) is @any {{\n{body}\nlet repl_kept is @any = [{}]; repl_kept; }} }}\n",
                self.header(),
                params.join(", "),
                keeps.join(", ")
            )
        };
        // Сначала — с печатью значения последнего выражения; не вышло (`void`,
        // действие) — без неё: тогда ошибки, если они есть, — самого ввода.
        let compiled = match shape.shown(&body) {
            Some(shown) => self.compile(Some(&source(&shown))).or_else(|_| self.compile(Some(&source(&body)))),
            None => self.compile(Some(&source(&body))),
        };
        let module = match compiled {
            Ok(module) => module,
            Err(errors) => {
                eprint!("{errors}");
                return;
            }
        };
        let args: Vec<Carried> = carried.iter().map(|index| self.vars[*index].value.clone()).collect();
        let _ = io::stdout().flush();
        let (result, trace) = run_cell(&module, &cell, KEEPER, &args);
        match result {
            Ok(values) => {
                let mut vars = Vec::new();
                for (name, value) in kept.into_iter().zip(values) {
                    if value.value.portable() {
                        vars.push(Var {
                            name,
                            ty: value.ty,
                            value: value.value,
                        });
                    } else {
                        eprintln!("note: `{name}` ({}) is not kept between inputs", value.ty);
                    }
                }
                self.vars = vars;
            }
            Err(error) => match okkama_runtime::report(&error) {
                Some(text) => {
                    eprint!("{text}");
                    if std::env::var("OKKAMA_TRACE").as_deref() != Ok("off") {
                        let trace = okkama_runtime::render_trace(&module, &trace);
                        eprint!("{}", shift_input_lines(&trace.replace(&format!("{cell}.run"), INPUT)));
                    }
                }
                None => eprintln!("okkama: {error}"),
            },
        }
    }
}

/// Что во вводе: имена `let` верхнего уровня и последнее выражение.
#[derive(Default)]
struct Shape {
    lets: Vec<String>,
    /// Байты последнего выражения-инструкции во вводе: (инструкция, выражение).
    last: Option<((usize, usize), (usize, usize))>,
}

impl Shape {
    /// `None` — ввод не разбирается (ошибку покажет компиляция ячейки).
    fn of(body: &str) -> Option<Self> {
        let prefix = "module repl v1.0.0;\nmain() is int {\n";
        let source = SourceFile::new("<shape>", format!("{prefix}{body}\n}}\n")).ok()?;
        let program = syntax::parse(&source, &ParseOptions::default()).ok()?;
        let Some(ast::Item::Action(main)) = program.items.first() else {
            return None;
        };
        let at = |span: okkama_compiler::source::Span| {
            (span.start as usize - prefix.len(), span.end as usize - prefix.len())
        };
        let mut shape = Self::default();
        for stmt in &main.body.stmts {
            if let ast::StmtKind::Let { name, .. } = &stmt.kind
                && !shape.lets.contains(&name.text)
            {
                shape.lets.push(name.text.clone());
            }
        }
        if let Some(stmt) = main.body.stmts.last()
            && let ast::StmtKind::Expr { expr, condition: None } = &stmt.kind
        {
            shape.last = Some((at(stmt.span), at(expr.span)));
        }
        Some(shape)
    }

    /// Ввод, где последнее выражение сохраняется и печатается.
    fn shown(&self, body: &str) -> Option<String> {
        let ((start, end), (expr_start, expr_end)) = self.last?;
        let expr = body.get(expr_start..expr_end)?;
        Some(format!(
            "{}let repl_value = {expr}; ReplShow().show(repl_value);{}",
            body.get(..start)?,
            body.get(end..)?
        ))
    }
}

/// Ввод закончен: скобки сбалансированы вне строк, символов и комментариев.
fn complete(text: &str) -> bool {
    let mut depth = 0i32;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' | '\'' => {
                while let Some(inner) = chars.next() {
                    if inner == '\\' {
                        chars.next();
                    } else if inner == ch {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for inner in chars.by_ref() {
                    if inner == '\n' {
                        break;
                    }
                }
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    depth <= 0
}

/// Номера строк файла ввода — как у пользователя: его первая строка служебная.
fn shift_input_lines(text: &str) -> String {
    let marker = format!("{INPUT}:");
    let mut out = String::new();
    let mut in_input = false;
    for line in text.split_inclusive('\n') {
        if let Some(at) = line.find(&marker) {
            in_input = true;
            let (head, rest) = line.split_at(at + marker.len());
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            match rest[..digits].parse::<u32>() {
                Ok(number) => {
                    out.push_str(head);
                    out.push_str(&number.saturating_sub(1).to_string());
                    out.push_str(&rest[digits..]);
                }
                Err(_) => out.push_str(line),
            }
            continue;
        }
        if line.contains(":") && !line.trim_start().starts_with(|c: char| c.is_ascii_digit() || c == '|') {
            in_input = line.contains(INPUT);
        }
        let trimmed = line.trim_start();
        let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
        if in_input && digits > 0 && trimmed[digits..].starts_with(" |") {
            let indent = &line[..line.len() - trimmed.len()];
            let number: u32 = trimmed[..digits].parse().unwrap_or(1);
            out.push_str(indent);
            out.push_str(&format!("{:>width$}", number.saturating_sub(1), width = digits));
            out.push_str(&trimmed[digits..]);
        } else {
            out.push_str(line);
        }
    }
    out
}
