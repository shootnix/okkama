//! Сведения о программе для редактора (сервер языка, `okkama lsp`).
//!
//! [`Inspection`] — результат одного прохода по открытому файлу: диагностики
//! (синтаксис, анализ, предупреждения), символы для наведения и перехода к
//! определению и структура файла. Позиции — в единицах протокола LSP:
//! строки с нуля, столбец в кодовых единицах UTF-16; внутри компилятора
//! по-прежнему байтовые смещения ([`Span`]).
//!
//! Анализ мягкий ([`crate::analysis::analyze_lenient`]): тела проверяются и
//! при ошибках, поэтому наведение работает в файле, где ещё есть ошибки
//! типов. Синтаксическая ошибка останавливает разбор — тогда есть только
//! диагностика.

mod complete;
mod index;
mod lint;
pub(crate) mod outline;
mod std_paths;
pub use std_paths::{StdCompletion, StdKind, std_completions};

pub use complete::{Context, builtin_members, keywords};
pub use index::build;
pub use lint::lint;
pub use outline::outline;

use crate::source::{SourceFile, Span};
use crate::syntax::{self, ParseOptions, ast};

/// Корень с тестами или fakes проверяется в тестовом режиме, чтобы их тела
/// тоже анализировались. Файл, который полностью не разбирается, — тоже:
/// тогда синтаксическая ошибка теста не прячется производственным разбором.
pub fn has_tests(source: &SourceFile) -> bool {
    let Ok(program) = syntax::parse(source, &ParseOptions::default()) else {
        return true;
    };
    program.items.iter().any(|item| match item {
        ast::Item::Test(_) => true,
        ast::Item::Object(decl) => decl
            .members
            .iter()
            .any(|member| matches!(member, ast::ObjectMember::Test(_))),
        ast::Item::Role(decl) => decl
            .members
            .iter()
            .any(|member| matches!(member, ast::RoleMember::Test(_) | ast::RoleMember::Fake(_))),
        _ => false,
    })
}

/// Позиция LSP: строка и столбец (UTF-16) с нуля.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Место в файле `path` (имя [`SourceFile`]: путь файла).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: String,
    pub range: Range,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub path: String,
    pub range: Range,
    pub severity: Severity,
    pub message: String,
    /// Правило линтера (`unused-let`, ...); у ошибок анализа — нет.
    pub code: Option<String>,
    /// Лишний код (неиспользуемое, недостижимое): редактор показывает его бледным.
    pub unnecessary: bool,
}

/// Имя или конструкция корневого файла: что показать при наведении и куда
/// ведёт переход к определению.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub span: Span,
    /// Markdown: блок кода с сигнатурой, затем документирующий комментарий.
    pub hover: String,
    pub definition: Option<Location>,
    /// Ключ типа значения (`Inspection::members`) для дополнения после `.`.
    pub ty: Option<String>,
    /// Локальная или параметр: все вхождения в этом файле, можно переименовать.
    pub local: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Method,
    Field,
    Variable,
    Object,
    Role,
    Enum,
    EnumMember,
    Function,
    Keyword,
    Module,
    /// Имя аргумента именованного вызова: `zip:`.
    Argument,
}

/// Вариант дополнения.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    /// Сигнатура или тип.
    pub detail: String,
    pub documentation: String,
    /// Вставляемый текст: snippet LSP (`name(${1:x})`), если `snippet`.
    pub insert: String,
    pub snippet: bool,
}

/// Тело корня: его локальные и объект `self`.
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub span: Span,
    /// Имя, тип, смещение объявления.
    pub locals: Vec<(String, String, u32)>,
    /// Ключ членов `self` (`self:Name`), если тело принадлежит объекту.
    pub object: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutlineKind {
    Object,
    Role,
    Enum,
    EnumCase,
    Field,
    Constructor,
    Action,
    Cap,
    Native,
    Test,
    Main,
    /// `const` модуля (#153).
    Constant,
}

/// Элемент структуры файла (`textDocument/documentSymbol`).
#[derive(Clone, Debug)]
pub struct OutlineItem {
    pub name: String,
    pub detail: String,
    pub kind: OutlineKind,
    pub range: Range,
    pub selection: Range,
    pub children: Vec<OutlineItem>,
}

#[derive(Clone, Debug, Default)]
pub struct Inspection {
    pub diagnostics: Vec<Diagnostic>,
    pub symbols: Vec<Symbol>,
    pub outline: Vec<OutlineItem>,
    /// Члены по ключу типа (`Money`, `@int`), `self:Name` (с приватными и
    /// полями), `static:Enum` (варианты), `namespace:alias`, `module:path`.
    pub members: std::collections::HashMap<String, Vec<Completion>>,
    pub scopes: Vec<Scope>,
    /// Имена области модуля: типы, natives, пространства имён.
    pub globals: Vec<Completion>,
    /// Корневой файл разобран: символы и структура соответствуют тексту.
    pub parsed: bool,
}

impl Inspection {
    /// Вхождения имени под курсором: все символы с тем же определением
    /// (и само определение, если оно в этом файле), по порядку.
    pub fn occurrences(&self, offset: u32) -> Vec<&Symbol> {
        let Some(found) = self.symbol_at(offset) else {
            return Vec::new();
        };
        let Some(definition) = &found.definition else {
            return vec![found];
        };
        let mut all: Vec<&Symbol> = self
            .symbols
            .iter()
            .filter(|symbol| symbol.definition.as_ref() == Some(definition))
            .collect();
        all.sort_by_key(|symbol| symbol.span.start);
        all.dedup_by_key(|symbol| symbol.span);
        all
    }

    /// Символ под курсором: самый узкий span, содержащий `offset` (конец
    /// включительно — курсор сразу после имени тоже на нём).
    pub fn symbol_at(&self, offset: u32) -> Option<&Symbol> {
        self.symbols
            .iter()
            .filter(|symbol| symbol.span.start <= offset && offset <= symbol.span.end)
            .min_by_key(|symbol| symbol.span.len())
    }
}

/// Единица столбца позиции LSP (`positionEncoding`, #48): UTF-16 — значение
/// по умолчанию протокола, UTF-8 и UTF-32 — по согласию клиента.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    #[default]
    Utf16,
    Utf32,
}

impl Encoding {
    fn units(self, ch: char) -> u32 {
        match self {
            Encoding::Utf8 => ch.len_utf8() as u32,
            Encoding::Utf16 => ch.len_utf16() as u32,
            Encoding::Utf32 => 1,
        }
    }
}

/// Кодировка позиций процесса: сервер языка согласует её один раз при
/// `initialize` ([`set_encoding`]); остальные — UTF-16.
static ENCODING: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);

pub fn set_encoding(encoding: Encoding) {
    ENCODING.store(encoding as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn encoding() -> Encoding {
    match ENCODING.load(std::sync::atomic::Ordering::Relaxed) {
        0 => Encoding::Utf8,
        2 => Encoding::Utf32,
        _ => Encoding::Utf16,
    }
}

/// Перевод байтовых смещений текста в позиции LSP и обратно.
#[derive(Debug)]
pub struct LineIndex<'a> {
    text: &'a str,
    starts: Vec<usize>,
    encoding: Encoding,
}

impl<'a> LineIndex<'a> {
    /// В кодировке процесса ([`encoding`]).
    pub fn new(text: &'a str) -> Self {
        Self::with_encoding(text, encoding())
    }

    pub fn with_encoding(text: &'a str, encoding: Encoding) -> Self {
        let mut starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(offset + 1);
            }
        }
        Self { text, starts, encoding }
    }

    /// Конец строки `line` без `\n` и без `\r` перед ним.
    fn line_end(&self, line: usize) -> usize {
        let end = self.starts.get(line + 1).map_or(self.text.len(), |next| next - 1);
        if end > self.starts[line] && self.text.as_bytes()[end - 1] == b'\r' {
            end - 1
        } else {
            end
        }
    }

    pub fn position(&self, offset: u32) -> Position {
        let mut offset = (offset as usize).min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let start = self.starts[line];
        // `\r` of CRLF belongs to the line break, not to the line.
        let offset = offset.min(self.line_end(line).max(start));
        let character = self.text[start..offset].chars().map(|ch| self.encoding.units(ch)).sum();
        Position { line: line as u32, character }
    }

    pub fn range(&self, span: Span) -> Range {
        Range {
            start: self.position(span.start),
            end: self.position(span.end),
        }
    }

    /// Смещение позиции LSP; столбец за концом строки — конец строки,
    /// столбец внутри символа — начало следующего символа.
    pub fn offset(&self, position: Position) -> u32 {
        let line = position.line as usize;
        if line >= self.starts.len() {
            return self.text.len() as u32;
        }
        let (start, end) = (self.starts[line], self.line_end(line));
        let mut units = 0u32;
        for (index, ch) in self.text[start..end].char_indices() {
            if units >= position.character {
                return (start + index) as u32;
            }
            units += self.encoding.units(ch);
        }
        end as u32
    }
}

/// Комментарий `//` непосредственно над объявлением, начинающимся в `offset`
/// (строки без `//`, пустая строка между — конец комментария): правило единой
/// модели документации (`crate::doc`, #512).
pub fn doc_comment(source: &SourceFile, offset: u32) -> String {
    crate::doc::comment_above(source, offset)
}

/// Markdown наведения: сигнатура в блоке `okkama`, затем документация.
pub(crate) fn hover_text(signature: &str, doc: &str) -> String {
    if doc.trim().is_empty() {
        format!("```okkama\n{signature}\n```")
    } else {
        format!("```okkama\n{signature}\n```\n\n{}", doc_markdown(doc))
    }
}

/// Комментарии std используют отступ в четыре пробела для примеров: это уже
/// блок кода markdown; остальное — абзацы.
fn doc_markdown(doc: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in doc.lines() {
        let code = line.starts_with("    ");
        if code && !in_code {
            out.push_str("```okkama\n");
            in_code = true;
        } else if !code && in_code && !line.trim().is_empty() {
            out.push_str("```\n");
            in_code = false;
        }
        if in_code {
            out.push_str(line.strip_prefix("    ").unwrap_or(line));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if in_code {
        out.push_str("```\n");
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests;
