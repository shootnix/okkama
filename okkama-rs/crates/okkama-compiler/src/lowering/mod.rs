//! Преобразование AST в OIR-1 (§19 спецификации `docs/compiler/OIR-1.md`).
//!
//! Lowering выполняет то, что нужно для типизированного IR поддерживаемого
//! подмножества: разрешение локальных имён, вывод и проверку типов, выбор
//! инструкций и явное управление потоком. Конструкции, которых IR ещё не
//! поддерживает, дают [`LoweringError::Unsupported`]; программы, нарушающие
//! правила языка, — [`LoweringError::Invalid`]. Ничего не пропускается молча.
//!
//! Поддержано: `main() is int`; `int`, `bool`, `string`; литералы, имена,
//! арифметика, битовые операции, сравнения, `!`, `&&`, `||`; `let` с
//! необязательным типом, вложенные блоки, `if` как оператор и выражение,
//! `loop`, `for` по диапазону, `next`, `last`, `return`, `confirm`, `panic`;
//! объекты с типизированными полями и defaults, полные и shorthand
//! конструкторы, actions с перегрузкой и `private`, successor-действия,
//! rebind, обязательство использования successor, `int/bool.string()`.

mod body;
mod builder;
mod ctx;
mod intrinsics;
mod modules;

pub use modules::{LoadedModule, ModuleLoader};

use std::fmt;

use crate::ir;
use crate::source::{SourceFile, Span};
use crate::syntax::ast;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoweringError {
    /// Корректная конструкция Okkama, которую IR пока не поддерживает.
    Unsupported { span: Span, message: String },
    /// Программа нарушает правило языка: имена, типы, управление потоком.
    Invalid { span: Span, message: String },
}

impl LoweringError {
    pub(crate) fn unsupported(span: Span, message: impl Into<String>) -> Self {
        Self::Unsupported {
            span,
            message: message.into(),
        }
    }

    pub(crate) fn invalid(span: Span, message: impl Into<String>) -> Self {
        Self::Invalid {
            span,
            message: message.into(),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Unsupported { span, .. } | Self::Invalid { span, .. } => *span,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Unsupported { message, .. } | Self::Invalid { message, .. } => message,
        }
    }
}

impl fmt::Display for LoweringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for LoweringError {}

pub(crate) type Result<T> = std::result::Result<T, LoweringError>;

/// Ошибка связывания программы с импортами.
#[derive(Clone, Debug)]
pub enum LinkError {
    /// Ошибка lowering; `source` — файл импортированного модуля, `None` — корень.
    Lowering {
        source: Option<SourceFile>,
        error: LoweringError,
    },
    /// Загрузчик не нашёл или не разобрал модуль: текст готов к выводу.
    Load(String),
}

/// Преобразует весь модуль: ни одна декларация или инструкция не пропускается.
/// Импорты не разрешаются и остаются в IR как неразрешённые (§2.1).
///
/// Тестовые объявления должны быть удалены заранее
/// ([`crate::syntax::prune::strip_test_items`]): production-модуль OIR их не содержит.
pub fn lower_program(program: &ast::Program) -> Result<ir::Module> {
    let modules = vec![crate::analysis::SourceModule {
        path: modules::module_path(program),
        program,
        imports: Vec::new(),
    }];
    let unresolved = program
        .imports
        .iter()
        .map(|import| ir::Import {
            path: import.path.literal.value.clone(),
            alias: import.alias.as_ref().map(|alias| alias.text.clone()),
            take: import.take.iter().map(|name| name.text.clone()).collect(),
            span: import.span,
        })
        .collect();
    lower_modules(modules, 1, program, unresolved, false)
        .map(|(module, _)| module)
        .map_err(|(_, error)| error)
}

/// Связывает программу с её импортами (загружает их через `loader`) и
/// преобразует всё в один модуль OIR. `root_name` — имя файла корня: от него
/// загрузчик ищет относительные импорты.
pub fn lower_linked(
    program: &ast::Program,
    root_name: &str,
    loader: &mut dyn ModuleLoader,
) -> std::result::Result<ir::Module, LinkError> {
    lower_linked_with_warnings(program, root_name, loader).map(|(module, _)| module)
}

/// [`lower_linked`] и предупреждения анализа корневого модуля.
pub fn lower_linked_with_warnings(
    program: &ast::Program,
    root_name: &str,
    loader: &mut dyn ModuleLoader,
) -> std::result::Result<(ir::Module, Vec<CheckError>), LinkError> {
    lower_roots(&[(program, root_name, None)], loader, false)
}

/// Файл многофайлового корневого модуля (§5.1): программа и её исходник.
#[derive(Clone, Copy, Debug)]
pub struct RootFile<'a> {
    pub program: &'a ast::Program,
    pub source: &'a SourceFile,
}

fn roots<'a>(files: &[RootFile<'a>]) -> Vec<modules::Root<'a>> {
    files
        .iter()
        .map(|file| (file.program, file.source.name(), Some(file.source)))
        .collect()
}

/// [`lower_linked_with_warnings`] для корня из нескольких файлов: они —
/// одно пространство объявлений, импорты у каждого свои. Ошибки и
/// предупреждения файлов корня несут их исходник.
pub fn lower_linked_files(
    files: &[RootFile<'_>],
    loader: &mut dyn ModuleLoader,
) -> std::result::Result<(ir::Module, Vec<CheckError>), LinkError> {
    lower_roots(&roots(files), loader, false)
}

/// [`lower_linked_files`] в тестовом режиме (§14): программы разобраны с
/// тестами (`prune::desugar_test_items`), модуль OIR перечисляет тесты корня.
pub fn lower_linked_tests(
    files: &[RootFile<'_>],
    loader: &mut dyn ModuleLoader,
) -> std::result::Result<(ir::Module, Vec<CheckError>), LinkError> {
    lower_roots(&roots(files), loader, true)
}

/// Исходник модуля `module` связанной программы (`None` — его рендерит вызывающий).
fn source_of(
    graph: &modules::Graph,
    roots: &[modules::Root<'_>],
    module: usize,
) -> Option<SourceFile> {
    match graph.loaded.get(module) {
        Some(loaded) => Some(loaded.source.clone()),
        None => roots
            .get(module - graph.loaded.len())
            .and_then(|(.., source)| source.cloned()),
    }
}

fn lower_roots(
    roots: &[modules::Root<'_>],
    loader: &mut dyn ModuleLoader,
    test_mode: bool,
) -> std::result::Result<(ir::Module, Vec<CheckError>), LinkError> {
    let graph = modules::load_graph(roots, loader)?;
    let sources = modules::source_modules(&graph, roots);
    let first_root = graph.loaded.len();
    lower_modules(sources, roots.len(), roots[0].0, Vec::new(), test_mode)
        .map(|(mut module, warnings)| {
            module.debug.first_root = first_root;
            module.debug.sources = (0..first_root + roots.len())
                .map(|index| source_of(&graph, roots, index).map(std::sync::Arc::new))
                .collect();
            let warnings = warnings
                .into_iter()
                .filter(|warning| warning.module >= first_root)
                .map(|error| CheckError {
                    source: source_of(&graph, roots, error.module),
                    error,
                })
                .collect();
            (module, warnings)
        })
        .map_err(|(module, error)| LinkError::Lowering {
            source: source_of(&graph, roots, module),
            error,
        })
}

/// Ошибка анализа с файлом, в котором она найдена (`None` — корень).
#[derive(Clone, Debug)]
pub struct CheckError {
    pub source: Option<SourceFile>,
    pub error: crate::analysis::SemanticError,
}

type CheckResult =
    std::result::Result<Vec<CheckError>, std::result::Result<Vec<CheckError>, String>>;

/// Загружает импорты и проверяет программу целиком, без lowering: все
/// ошибки анализа (`okkama check`). `Ok` — предупреждения корневого модуля;
/// `Err` с пустым списком не бывает.
pub fn check_linked(
    program: &ast::Program,
    root_name: &str,
    loader: &mut dyn ModuleLoader,
) -> CheckResult {
    check_roots(&[(program, root_name, None)], loader, false)
}

/// [`check_linked`] для корня из нескольких файлов (§5.1).
pub fn check_linked_files(files: &[RootFile<'_>], loader: &mut dyn ModuleLoader) -> CheckResult {
    check_roots(&roots(files), loader, false)
}

/// [`check_linked_files`] в тестовом режиме (§14).
pub fn check_linked_tests(files: &[RootFile<'_>], loader: &mut dyn ModuleLoader) -> CheckResult {
    check_roots(&roots(files), loader, true)
}

fn check_roots(
    roots: &[modules::Root<'_>],
    loader: &mut dyn ModuleLoader,
    test_mode: bool,
) -> CheckResult {
    let graph = modules::load_graph(roots, loader).map_err(|error| match error {
        LinkError::Load(text) => Err(text),
        LinkError::Lowering { source, error } => Ok(vec![CheckError {
            source,
            error: crate::analysis::SemanticError::invalid(0, error.span(), error.message()),
        }]),
    })?;
    let sources = modules::source_modules(&graph, roots);
    let first_root = graph.loaded.len();
    crate::analysis::analyze_mode(sources, roots.len(), test_mode)
        .map(|analysis| {
            analysis
                .warnings
                .into_iter()
                .filter(|warning| warning.module >= first_root)
                .map(|error| CheckError {
                    source: source_of(&graph, roots, error.module),
                    error,
                })
                .collect()
        })
        .map_err(|errors| {
            Ok(errors
                .into_iter()
                .map(|error| CheckError {
                    source: source_of(&graph, roots, error.module),
                    error,
                })
                .collect())
        })
}

/// Анализ, затем lowering проверенной программы. Первой сообщается ошибка
/// программы, а не ограничение реализации (`Unsupported` — только если других нет).
fn lower_modules(
    modules: Vec<crate::analysis::SourceModule<'_>>,
    roots: usize,
    program: &ast::Program,
    imports: Vec<ir::Import>,
    test_mode: bool,
) -> std::result::Result<(ir::Module, Vec<crate::analysis::SemanticError>), (usize, LoweringError)>
{
    let analysis = crate::analysis::analyze_mode(modules, roots, test_mode).map_err(|errors| {
        let first = errors
            .iter()
            .find(|error| !error.unsupported)
            .unwrap_or(&errors[0]);
        let error = if first.unsupported {
            LoweringError::unsupported(first.span, first.message.clone())
        } else {
            LoweringError::invalid(first.span, first.message.clone())
        };
        (first.module, error)
    })?;
    let root = analysis.registry.entry_module;
    let mut ctx = ctx::Ctx::new(&analysis.registry, &analysis.bodies);
    ctx.collect().map_err(|error| (root, error))?;
    if test_mode {
        ctx.collect_tests();
    }
    // Очередь растёт, пока тела запрашивают специализации и shorthand-формы.
    let mut functions = Vec::new();
    let mut next = 0;
    while next < ctx.tasks.len() {
        let module = ctx.task_module(next);
        functions
            .push(body::lower_task(&mut ctx, ir::FuncId(next)).map_err(|error| (module, error))?);
        next += 1;
    }
    let owners: Vec<usize> = (0..functions.len()).map(|index| ctx.task_module(index)).collect();
    let mut module = ctx.module(program, imports, functions);
    module.debug.function_modules = owners.clone();
    // Вызов, который не может завершиться ничем, кроме рекурсии, — ошибка.
    let infinite = ir::recursion::infinite(&module);
    if let Some(func) = infinite.first() {
        let name = |id: &ir::FuncId| {
            let symbol = &module.functions[id.0].symbol;
            format!("\"{}\"", symbol.split('#').next().unwrap_or(symbol))
        };
        let others: Vec<String> = infinite.iter().filter(|other| *other != func).map(name).collect();
        let message = if others.is_empty() {
            format!("Infinite recursion: {} cannot complete without calling itself", name(func))
        } else {
            format!(
                "Infinite recursion: {} cannot complete without recursive calls through {}",
                name(func),
                others.join(", ")
            )
        };
        return Err((owners[func.0], LoweringError::invalid(module.functions[func.0].span, message)));
    }
    // Хвостовая рекурсия исполняется циклом на всех движках.
    ir::recursion::eliminate_tail_calls(&mut module);
    // Lowering обязан выдавать только корректный IR: нарушение — ошибка компилятора.
    if let Err(error) = ir::verify(&module) {
        // Отладка компилятора: печать отвергнутого IR.
        if let Ok(path) = std::env::var("OKKAMA_DUMP_INVALID_IR") {
            let _ = std::fs::write(path, module.to_string());
        }
        return Err((
            root,
            LoweringError::invalid(
                program.span,
                format!("internal compiler error: lowering produced invalid IR: {error}"),
            ),
        ));
    }
    Ok((module, analysis.warnings.clone()))
}

/// Редактор (`okkama lsp`): разбор корня, загрузка импортов и мягкий анализ.
/// Диагностики — все ошибки и предупреждения корня (ошибки импортированных
/// модулей — в их файлах), символы и структура — для корня. `test_mode` —
/// корень с тестами: тела тестов тоже проверяются (§14).
pub fn inspect(
    source: &SourceFile,
    loader: &mut dyn ModuleLoader,
    test_mode: bool,
) -> crate::ide::Inspection {
    use crate::ide::{Diagnostic, Inspection, LineIndex, Severity};
    use crate::syntax::{self, ParseMode, ParseOptions, prune};

    let lines = LineIndex::new(source.text());
    let options = ParseOptions {
        mode: if test_mode {
            ParseMode::Full
        } else {
            ParseMode::Production
        },
        ..ParseOptions::default()
    };
    let parsed = match syntax::parse(source, &options) {
        Ok(program) => program,
        Err(diagnostics) => {
            return Inspection {
                diagnostics: diagnostics
                    .iter()
                    .map(|found| Diagnostic {
                        path: source.name().to_string(),
                        range: lines.range(found.span),
                        severity: Severity::Error,
                        message: found.message.clone(),
                        code: None,
                        unnecessary: false,
                    })
                    .collect(),
                ..Inspection::default()
            };
        }
    };
    let outline = crate::ide::outline(&parsed, &lines);
    let program = if test_mode {
        prune::desugar_test_items(parsed)
    } else {
        prune::strip_test_items(parsed)
    };
    let mut inspection = Inspection {
        outline,
        parsed: true,
        ..Inspection::default()
    };
    let roots: [modules::Root<'_>; 1] = [(&program, source.name(), Some(source))];
    let graph = match modules::load_graph(&roots, loader) {
        Ok(graph) => graph,
        Err(LinkError::Load(text)) => {
            let span = program
                .imports
                .first()
                .map_or(Span::empty(0), |import| import.span);
            inspection.diagnostics.push(Diagnostic {
                path: source.name().to_string(),
                range: lines.range(span),
                severity: Severity::Error,
                message: text.trim_end().to_string(),
                code: None,
                unnecessary: false,
            });
            return inspection;
        }
        Err(LinkError::Lowering {
            source: file,
            error,
        }) => {
            let file = file.unwrap_or_else(|| source.clone());
            inspection.diagnostics.push(Diagnostic {
                path: file.name().to_string(),
                range: LineIndex::new(file.text()).range(error.span()),
                severity: Severity::Error,
                message: error.message().to_string(),
                code: None,
                unnecessary: false,
            });
            return inspection;
        }
    };
    let files: Vec<SourceFile> = graph
        .loaded
        .iter()
        .map(|loaded| loaded.source.clone())
        .chain(std::iter::once(source.clone()))
        .collect();
    let root = files.len() - 1;
    let sources = modules::source_modules(&graph, &roots);
    let (registry, bodies) = crate::analysis::analyze_lenient(sources, 1, test_mode);
    let mut indexes: std::collections::HashMap<usize, LineIndex<'_>> =
        std::collections::HashMap::new();
    let mut report = |error: &crate::analysis::SemanticError, severity: Severity| {
        let Some(file) = files.get(error.module) else {
            return;
        };
        let range = indexes
            .entry(error.module)
            .or_insert_with(|| LineIndex::new(file.text()))
            .range(error.span);
        let message = if error.unsupported {
            format!("analysis limitation: {}", error.message)
        } else {
            error.message.clone()
        };
        inspection.diagnostics.push(Diagnostic {
            path: file.name().to_string(),
            range,
            severity,
            message,
            code: None,
            unnecessary: false,
        });
    };
    for error in &registry.errors {
        report(error, Severity::Error);
    }
    for warning in registry
        .warnings
        .iter()
        .filter(|warning| warning.module == root)
    {
        report(warning, Severity::Warning);
    }
    inspection
        .diagnostics
        .extend(crate::ide::lint(&registry, &bodies, source, root));
    let index = crate::ide::build(&registry, &bodies, &files, root);
    inspection.symbols = index.symbols;
    inspection.members = index.members;
    inspection.scopes = index.scopes;
    inspection.globals = index.globals;
    inspection
}
