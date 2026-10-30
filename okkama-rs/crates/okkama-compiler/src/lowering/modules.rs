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

    /// Модуль `path` только из стандартной библиотеки (пути `std.…`).
    /// Загрузчик без отдельного поиска std ищет его, как обычный импорт.
    fn load_std(&mut self, path: &str, importer: &str) -> Result<Option<LoadedModule>, String> {
        self.load(path, importer)
    }

    /// Разрешён ли импорт `path` из файла `importer` до поиска файла
    /// (например, `lib/…` — только прямые зависимости проекта); `Err` —
    /// текст ошибки у импорта.
    fn check_import(&mut self, _path: &str, _importer: &str) -> Result<(), String> {
        Ok(())
    }

    /// Файл принадлежит blade-проекту: версия модуля задаётся в `blade.toml`,
    /// а не в объявлении `module`.
    fn in_project(&mut self, _file: &str) -> bool {
        false
    }
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
        walk.unversioned(program, name, *source)?;
        root_imports.push(walk.imports_of(program, name, *source)?);
    }
    walk.imports.extend(root_imports);
    // Имена объявлений квалифицируются именем модуля: два разных файла с
    // одним `module` (локальный модуль, затеняющий std, и сам std через
    // `std.…` или импорт внутри std) в одной программе не уживутся.
    let mut seen: HashMap<String, &LoadedModule> = HashMap::new();
    for loaded in &walk.loaded {
        let name = module_path(&loaded.program);
        if let Some(first) = seen.get(&name) {
            return Err(LinkError::Lowering {
                source: Some(loaded.source.clone()),
                error: LoweringError::invalid(
                    loaded.program.module.span,
                    format!(
                        "Module \"{name}\" is loaded from two files: {} and {}; a program can use only one",
                        first.id, loaded.id
                    ),
                ),
            });
        }
        seen.insert(name, loaded);
    }
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
    /// В blade-проекте версия модуля — в `blade.toml`, не в файле.
    fn unversioned(
        &mut self,
        program: &ast::Program,
        name: &str,
        source: Option<&SourceFile>,
    ) -> Result<(), LinkError> {
        match &program.module.version {
            Some(version) if self.loader.in_project(name) => Err(LinkError::Lowering {
                source: source.cloned(),
                error: LoweringError::invalid(
                    version.span,
                    format!(
                        "Module version {} is not allowed in a blade project: the version is set in blade.toml",
                        version.text()
                    ),
                ),
            }),
            _ => Ok(()),
        }
    }

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
            let std_module = path.strip_prefix(super::std_paths::STD_IMPORT);
            if std_module.is_none() {
                if let Err(message) = self.loader.check_import(&path, name) {
                    return Err(LinkError::Lowering {
                        source: source.cloned(),
                        error: LoweringError::invalid(import.path.span, message),
                    });
                }
            }
            let found = match std_module {
                Some(module) => self.loader.load_std(module, name),
                None => self.loader.load(&path, name),
            };
            let Some(mut module) = found.map_err(LinkError::Load)? else {
                let message = match std_module {
                    Some(module) => format!(
                        "Standard library module \"{module}\" not found (std.{})",
                        module.replace('/', ".")
                    ),
                    None => format!("Import \"{path}\" not found"),
                };
                return Err(LinkError::Lowering {
                    source: source.cloned(),
                    error: LoweringError::invalid(import.span, message),
                });
            };
            super::std_paths::desugar(&mut module.program).map_err(|error| {
                LinkError::Lowering {
                    source: Some(module.source.clone()),
                    error,
                }
            })?;
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
            self.unversioned(&module.program, &module.id, Some(&module.source))?;
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
