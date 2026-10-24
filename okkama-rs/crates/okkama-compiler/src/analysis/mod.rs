//! Семантический анализ: всё, что можно проверить до исполнения.
//!
//! Вход — разобранные модули программы (корень последним, зависимости
//! раньше, см. [`SourceModule`]). Выход — [`Analysis`]: таблица объявлений
//! ([`registry::Registry`]) и типизированные тела ([`thir::Body`]) всех
//! функций, либо полный список ошибок.
//!
//! Этапы:
//!
//! 1. объявления ([`registry`]): имена модулей, импорты (`take`, `as`,
//!    пространства имён), сигнатуры объектов, ролей, enum и natives,
//!    конструкторы, перегрузки, проекции, соответствие ролям, ограничения
//!    generic-параметров;
//! 2. тела ([`check`]): разрешение имён, типы каждого выражения, выбор
//!    перегрузок и конструкторов, вывод аргументов generic-конструирования,
//!    явные приведения (R02: разные числовые категории без каста не
//!    смешиваются), границы ролей и `any`, правила `return`/`next`/`last`,
//!    `self`, приватности, `throw`/`catch`, async/await.
//!
//! Тела generic-объявлений проверяются один раз, символьно: параметр типа —
//! непрозрачный тип, доступный только через свою роль-ограничение.
//! Результат анализа не зависит от того, что умеет backend.

pub mod check;
mod flow;
pub mod registry;
pub mod thir;
pub mod ty;

use std::fmt;

pub use registry::{Registry, SourceModule};

use crate::source::Span;

/// Ошибка анализа в файле модуля `module`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticError {
    pub module: usize,
    pub span: Span,
    pub message: String,
    /// Конструкция корректна, но анализ её ещё не поддерживает.
    pub unsupported: bool,
    /// Предупреждение: программа корректна, но, возможно, не то, что задумано.
    pub warning: bool,
}

impl SemanticError {
    pub fn invalid(module: usize, span: Span, message: impl Into<String>) -> Self {
        Self {
            module,
            span,
            message: message.into(),
            unsupported: false,
            warning: false,
        }
    }

    pub fn warning(module: usize, span: Span, message: impl Into<String>) -> Self {
        Self {
            module,
            span,
            message: message.into(),
            unsupported: false,
            warning: true,
        }
    }

    pub fn unsupported(module: usize, span: Span, message: impl Into<String>) -> Self {
        Self {
            module,
            span,
            message: message.into(),
            unsupported: true,
            warning: false,
        }
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Проверенная программа.
#[derive(Debug)]
pub struct Analysis<'a> {
    pub registry: Registry<'a>,
    pub bodies: Vec<thir::Body>,
    /// Предупреждения всех модулей программы.
    pub warnings: Vec<SemanticError>,
}

/// Анализ программы. Ошибки возвращаются все, в порядке обнаружения:
/// сначала объявления, затем тела в порядке исходника.
pub fn analyze(sources: Vec<SourceModule<'_>>) -> Result<Analysis<'_>, Vec<SemanticError>> {
    analyze_files(sources, 1)
}

/// [`analyze`] многофайлового корня: последние `roots` источников — файлы
/// одного модуля (§5.1).
pub fn analyze_files(
    sources: Vec<SourceModule<'_>>,
    roots: usize,
) -> Result<Analysis<'_>, Vec<SemanticError>> {
    analyze_mode(sources, roots, false)
}

/// [`analyze_files`] в тестовом режиме (§14): тесты корня и fakes.
pub fn analyze_mode(
    sources: Vec<SourceModule<'_>>,
    roots: usize,
    test_mode: bool,
) -> Result<Analysis<'_>, Vec<SemanticError>> {
    let mut registry = Registry::build_mode(sources, roots, test_mode);
    let bodies = check::check_bodies(&mut registry);
    if registry.errors.is_empty() {
        let warnings = std::mem::take(&mut registry.warnings);
        Ok(Analysis {
            registry,
            bodies,
            warnings,
        })
    } else {
        Err(std::mem::take(&mut registry.errors))
    }
}

/// Анализ для редактора: тела проверяются и при ошибках, ошибки и
/// предупреждения остаются в `registry.errors`/`registry.warnings`.
pub fn analyze_lenient(
    sources: Vec<SourceModule<'_>>,
    roots: usize,
    test_mode: bool,
) -> (Registry<'_>, Vec<thir::Body>) {
    let mut registry = Registry::build_mode(sources, roots, test_mode);
    let bodies = check::check_bodies(&mut registry);
    (registry, bodies)
}
