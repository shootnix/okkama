//! Загрузка импортов и связывание модулей программы (OIR-1 §2).
//!
//! `import "path" take A, B;` ищет модуль через [`ModuleLoader`] (порядок
//! поиска задаёт загрузчик; CLI повторяет Perl: каталоги импорта, каталог
//! импортирующего файла, std). Граф обходится в глубину: зависимости
//! получают меньшие индексы, корень — последний, цикл — ошибка с текстом
//! Perl. Связанная программа становится одним модулем OIR: объявления
//! импортированных модулей квалифицированы (`io.stdout::Stdout`).

use std::collections::{HashMap, HashSet};

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
        on_path: HashSet::new(),
        bytes: 0,
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

/// Наибольшая глубина вложенных импортов (корень → … → модуль).
pub const IMPORT_DEPTH: usize = 1024;
/// Наибольшее число загруженных модулей программы.
pub const MODULE_COUNT: usize = 16384;
/// Наибольший суммарный размер исходников загруженных модулей.
pub const MODULE_BYTES: usize = 256 << 20;

struct Walk<'l> {
    loader: &'l mut dyn ModuleLoader,
    loaded: Vec<LoadedModule>,
    imports: Vec<Vec<(usize, usize)>>,
    cache: HashMap<String, usize>,
    /// Пути импортов на текущем пути обхода (для текста о цикле) и id.
    active: Vec<(String, String)>,
    /// id модулей `active`: проверка цикла без линейного поиска.
    on_path: HashSet<String>,
    bytes: usize,
}

/// Программа, импорты которой обходятся: файл корня или загруженный модуль.
enum Owner<'a> {
    Root(&'a ast::Program, &'a str, Option<&'a SourceFile>),
    Loaded(LoadedModule),
}

/// Кадр обхода: следующий импорт программы и уже связанные.
struct Frame<'a> {
    owner: Owner<'a>,
    next: usize,
    result: Vec<(usize, usize)>,
    /// Номер `import` в программе родителя, который привёл сюда.
    parent_import: usize,
}

impl Frame<'_> {
    fn program(&self) -> &ast::Program {
        match &self.owner {
            Owner::Root(program, ..) => program,
            Owner::Loaded(module) => &module.program,
        }
    }

    fn name(&self) -> &str {
        match &self.owner {
            Owner::Root(_, name, _) => name,
            Owner::Loaded(module) => &module.id,
        }
    }

    fn source(&self) -> Option<SourceFile> {
        match &self.owner {
            Owner::Root(_, _, source) => source.cloned(),
            Owner::Loaded(module) => Some(module.source.clone()),
        }
    }
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
    /// (у корня `None`: ошибки корня рендерит вызывающий). Обход в глубину
    /// с явным стеком: глубина графа не расходует стек потока.
    fn imports_of(
        &mut self,
        program: &ast::Program,
        name: &str,
        source: Option<&SourceFile>,
    ) -> Result<Vec<(usize, usize)>, LinkError> {
        let mut stack = vec![Frame {
            owner: Owner::Root(program, name, source),
            next: 0,
            result: Vec::new(),
            parent_import: 0,
        }];
        loop {
            let frame = stack.last_mut().expect("root frame");
            if frame.next == frame.program().imports.len() {
                let done = stack.pop().expect("frame");
                let Owner::Loaded(module) = done.owner else {
                    return Ok(done.result);
                };
                self.active.pop();
                self.on_path.remove(&module.id);
                let position = self.loaded.len();
                self.cache.insert(module.id.clone(), position);
                self.loaded.push(module);
                self.imports.push(done.result);
                stack.last_mut().expect("parent frame").result.push((position, done.parent_import));
                continue;
            }
            let index = frame.next;
            frame.next += 1;
            let frame = stack.last().expect("frame");
            let import = &frame.program().imports[index];
            let (path, path_span, span) = (import.path.literal.value.clone(), import.path.span, import.span);
            let name = frame.name().to_string();
            let fail = |stack: &[Frame<'_>], span, message: String| LinkError::Lowering {
                source: stack.last().expect("frame").source(),
                error: LoweringError::invalid(span, message),
            };
            let std_module = path.strip_prefix(super::std_paths::STD_IMPORT);
            if std_module.is_none() {
                if let Err(message) = self.loader.check_import(&path, &name) {
                    return Err(fail(&stack, path_span, message));
                }
            }
            let found = match std_module {
                Some(module) => self.loader.load_std(module, &name),
                None => self.loader.load(&path, &name),
            };
            let Some(mut module) = found.map_err(LinkError::Load)? else {
                let message = match std_module {
                    Some(module) => format!(
                        "Standard library module \"{module}\" not found (std.{})",
                        module.replace('/', ".")
                    ),
                    None => format!("Import \"{path}\" not found"),
                };
                return Err(fail(&stack, span, message));
            };
            super::std_paths::desugar(&mut module.program).map_err(|error| {
                LinkError::Lowering {
                    source: Some(module.source.clone()),
                    error,
                }
            })?;
            if self.on_path.contains(&module.id) {
                let position = self.active.iter().position(|(_, id)| *id == module.id).expect("on path");
                let mut chain: Vec<String> = self.active[position..]
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect();
                chain.push(path);
                return Err(fail(&stack, span, format!("Import cycle detected: {}", chain.join(" -> "))));
            }
            if let Some(done) = self.cache.get(&module.id) {
                stack.last_mut().expect("frame").result.push((*done, index));
                continue;
            }
            if self.active.len() >= IMPORT_DEPTH {
                return Err(fail(
                    &stack,
                    span,
                    format!("Import \"{path}\" nests deeper than {IMPORT_DEPTH} modules"),
                ));
            }
            self.bytes += module.source.len() as usize;
            if self.loaded.len() + self.active.len() >= MODULE_COUNT || self.bytes > MODULE_BYTES {
                return Err(fail(
                    &stack,
                    span,
                    format!(
                        "Import \"{path}\" exceeds the program limit of {MODULE_COUNT} modules or {} MiB of module sources",
                        MODULE_BYTES >> 20
                    ),
                ));
            }
            self.unversioned(&module.program, &module.id, Some(&module.source))?;
            self.active.push((path, module.id.clone()));
            self.on_path.insert(module.id.clone());
            stack.push(Frame {
                owner: Owner::Loaded(module),
                next: 0,
                result: Vec::new(),
                parent_import: index,
            });
        }
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
