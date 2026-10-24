//! Загрузка импортов и связывание модулей программы (OIR-1 §2).
//!
//! `import "path" take A, B;` ищет модуль через [`ModuleLoader`] (порядок
//! поиска задаёт загрузчик; CLI повторяет Perl: каталоги импорта, каталог
//! импортирующего файла, std). Граф обходится в глубину: зависимости
//! получают меньшие индексы, корень — последний, цикл — ошибка с текстом
//! Perl. Связанная программа становится одним модулем OIR: объявления
//! импортированных модулей квалифицированы (`io.stdout::Stdout`).

use std::collections::HashMap;

use super::{LinkError, LoweringError};
use crate::analysis::SourceModule;
use crate::source::SourceFile;
use crate::syntax::ast;

/// Найденный и разобранный модуль.
#[derive(Debug)]
pub struct LoadedModule {
    /// Каноническое имя (путь файла): один модуль загружается один раз.
    pub id: String,
    pub source: SourceFile,
    /// Программа без тестовых объявлений.
    pub program: ast::Program,
}

/// Поставщик импортируемых модулей.
pub trait ModuleLoader {
    /// Модуль `path`, импортированный из файла `importer`; `Ok(None)` — не
    /// найден. `Err` — готовый к выводу текст (например, синтаксическая
    /// ошибка в найденном файле).
    fn load(&mut self, path: &str, importer: &str) -> Result<Option<LoadedModule>, String>;
}

/// Загруженный граф: модули в порядке зависимостей и импорты каждого
/// (индекс зависимости, индекс `import` в программе модуля); последние
/// записи `imports` — файлы корня.
pub(super) struct Graph {
    pub loaded: Vec<LoadedModule>,
    pub imports: Vec<Vec<(usize, usize)>>,
}

/// Файл корня: программа, имя файла (от него ищутся импорты) и исходник
/// (`None` — ошибки рендерит вызывающий).
pub(super) type Root<'a> = (&'a ast::Program, &'a str, Option<&'a SourceFile>);

pub(super) fn load_graph(
    roots: &[Root<'_>],
    loader: &mut dyn ModuleLoader,
) -> Result<Graph, LinkError> {
    let mut walk = Walk {
        loader,
        loaded: Vec::new(),
        imports: Vec::new(),
        cache: HashMap::new(),
        active: Vec::new(),
    };
    let mut root_imports = Vec::new();
    for (program, name, source) in roots {
        root_imports.push(walk.imports_of(program, name, *source)?);
    }
    walk.imports.extend(root_imports);
    Ok(Graph {
        loaded: walk.loaded,
        imports: walk.imports,
    })
}

struct Walk<'l> {
    loader: &'l mut dyn ModuleLoader,
    loaded: Vec<LoadedModule>,
    imports: Vec<Vec<(usize, usize)>>,
    cache: HashMap<String, usize>,
    /// Пути импортов на текущем пути обхода (для текста о цикле) и id.
    active: Vec<(String, String)>,
}

impl Walk<'_> {
    /// Импорты программы `program` из файла `name`; `source` — её исходник
    /// (у корня `None`: ошибки корня рендерит вызывающий).
    fn imports_of(
        &mut self,
        program: &ast::Program,
        name: &str,
        source: Option<&SourceFile>,
    ) -> Result<Vec<(usize, usize)>, LinkError> {
        let mut result = Vec::new();
        for (index, import) in program.imports.iter().enumerate() {
            let path = import.path.literal.value.clone();
            let Some(module) = self.loader.load(&path, name).map_err(LinkError::Load)? else {
                return Err(LinkError::Lowering {
                    source: source.cloned(),
                    error: LoweringError::invalid(
                        import.span,
                        format!("Import \"{path}\" not found"),
                    ),
                });
            };
            if let Some(position) = self.active.iter().position(|(_, id)| *id == module.id) {
                let mut chain: Vec<String> = self.active[position..]
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect();
                chain.push(path);
                return Err(LinkError::Lowering {
                    source: source.cloned(),
                    error: LoweringError::invalid(
                        import.span,
                        format!("Import cycle detected: {}", chain.join(" -> ")),
                    ),
                });
            }
            if let Some(done) = self.cache.get(&module.id) {
                result.push((*done, index));
                continue;
            }
            self.active.push((path, module.id.clone()));
            let nested = self.imports_of(&module.program, &module.id, Some(&module.source))?;
            self.active.pop();
            let position = self.loaded.len();
            self.cache.insert(module.id.clone(), position);
            self.loaded.push(module);
            self.imports.push(nested);
            result.push((position, index));
        }
        Ok(result)
    }
}

/// Исходные модули для анализа: импортированные, затем файлы корня.
pub(super) fn source_modules<'a>(graph: &'a Graph, roots: &[Root<'a>]) -> Vec<SourceModule<'a>> {
    let mut modules = Vec::new();
    for (index, loaded) in graph.loaded.iter().enumerate() {
        modules.push(source_module(&loaded.program, &graph.imports[index]));
    }
    for (position, (program, ..)) in roots.iter().enumerate() {
        modules.push(source_module(
            program,
            &graph.imports[graph.loaded.len() + position],
        ));
    }
    modules
}

fn source_module<'a>(program: &'a ast::Program, imports: &[(usize, usize)]) -> SourceModule<'a> {
    SourceModule {
        path: module_path(program),
        program,
        imports: imports
            .iter()
            .map(|(dependency, index)| (*dependency, &program.imports[*index]))
            .collect(),
    }
}

pub(super) fn module_path(program: &ast::Program) -> String {
    program
        .module
        .path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join("/")
}
