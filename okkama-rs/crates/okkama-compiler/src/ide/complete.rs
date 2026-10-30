//! Дополнение и подсказка сигнатуры.
//!
//! Во время набора текст обычно не разбирается (`receiver.` без имени),
//! поэтому типы берутся из последней успешной проверки, а контекст — из
//! текущего текста: позиция в нём переносится в проверенный текст по
//! общему началу или общему концу двух текстов (правка обычно одна и рядом
//! с курсором).

use super::{Completion, CompletionKind, Inspection};

/// Что дополнять в позиции.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Context {
    /// После `receiver.`: начало и конец «якоря» получателя (имя, имя вызова
    /// перед `(`, строковый литерал).
    Member { anchor: (u32, u32), literal: bool },
    /// Имена из `take` импорта `path`.
    Take { path: String },
    /// Имя в выражении или объявлении.
    Plain,
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Начало имени, которое дописывается в `offset`.
pub fn prefix_start(text: &str, offset: u32) -> u32 {
    let bytes = text.as_bytes();
    let mut start = (offset as usize).min(bytes.len());
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    start as u32
}

impl Context {
    pub fn at(text: &str, offset: u32) -> Context {
        let bytes = text.as_bytes();
        let start = prefix_start(text, offset) as usize;
        let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
        let line = text[line_start..start].trim_start();
        if let Some(rest) = line.strip_prefix("import \"") {
            if let Some((path, tail)) = rest.split_once('"') {
                if tail.contains(" take ") || tail.trim_end().ends_with("take") {
                    return Context::Take {
                        path: path.to_string(),
                    };
                }
            }
        }
        let mut dot = start;
        while dot > 0 && bytes[dot - 1] == b' ' {
            dot -= 1;
        }
        if dot == 0 || bytes[dot - 1] != b'.' || (dot >= 2 && bytes[dot - 2] == b'.') {
            return Context::Plain;
        }
        let mut end = dot - 1;
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        if end == 0 {
            return Context::Plain;
        }
        match bytes[end - 1] {
            b')' => match matching_open(bytes, end - 1, b'(', b')') {
                Some(open) => {
                    let name_end = open;
                    let mut name_start = name_end;
                    while name_start > 0 && is_ident(bytes[name_start - 1]) {
                        name_start -= 1;
                    }
                    if name_start == name_end {
                        Context::Plain
                    } else {
                        Context::Member {
                            anchor: (name_start as u32, name_end as u32),
                            literal: false,
                        }
                    }
                }
                None => Context::Plain,
            },
            b'"' => Context::Member {
                anchor: ((end - 1) as u32, end as u32),
                literal: true,
            },
            byte if is_ident(byte) => {
                let mut name_start = end;
                while name_start > 0 && is_ident(bytes[name_start - 1]) {
                    name_start -= 1;
                }
                if bytes[name_start].is_ascii_digit() {
                    return Context::Plain;
                }
                Context::Member {
                    anchor: (name_start as u32, end as u32),
                    literal: false,
                }
            }
            _ => Context::Plain,
        }
    }
}

/// Позиция открывающей скобки для закрывающей в `close` (строки не учитываются).
fn matching_open(bytes: &[u8], close: usize, open: u8, shut: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = close + 1;
    while index > 0 {
        index -= 1;
        if bytes[index] == shut {
            depth += 1;
        } else if bytes[index] == open {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// Имя, которое начинается в `at`.
fn name_at(text: &str, at: usize) -> &str {
    let rest = text.get(at..).unwrap_or("");
    &rest[..rest.bytes().take_while(|byte| is_ident(*byte)).count()]
}

/// Смещение `offset` текущего текста в проверенном (`analyzed`).
pub fn map_offset(analyzed: &str, text: &str, offset: u32) -> Option<u32> {
    if analyzed == text {
        return Some(offset);
    }
    let (a, b) = (analyzed.as_bytes(), text.as_bytes());
    // Общие начало и конец не режут имя: `obj` и `out` не совпадают в `o`.
    let mut prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let split = |at: usize| {
        a.get(at).is_some_and(|byte| is_ident(*byte))
            || b.get(at).is_some_and(|byte| is_ident(*byte))
    };
    while prefix > 0 && is_ident(b[prefix - 1]) && split(prefix) {
        prefix -= 1;
    }
    if (offset as usize) <= prefix {
        return Some(offset);
    }
    let limit = a.len().min(b.len()) - prefix;
    let mut suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(limit)
        .take_while(|(x, y)| x == y)
        .count();
    let joined =
        |text: &[u8], suffix: usize| text.len() > suffix && is_ident(text[text.len() - suffix - 1]);
    while suffix > 0 && is_ident(b[b.len() - suffix]) && (joined(a, suffix) || joined(b, suffix)) {
        suffix -= 1;
    }
    if offset as usize >= b.len() - suffix {
        return Some((offset as usize + a.len() - b.len()) as u32);
    }
    None
}

/// Позиция для поиска областей видимости: перенесённая или начало правки.
fn scope_offset(analyzed: &str, text: &str, offset: u32) -> u32 {
    map_offset(analyzed, text, offset).unwrap_or_else(|| {
        analyzed
            .bytes()
            .zip(text.bytes())
            .take_while(|(a, b)| a == b)
            .count() as u32
    })
}

/// Тип результата из подписи `(a is int) is R` (без `^` successor).
fn result_type(detail: &str) -> Option<String> {
    let (_, result) = detail.rsplit_once(" is ")?;
    let result = result.trim().trim_start_matches('^');
    (result != "void").then(|| result.to_string())
}

impl Inspection {
    /// Члены типа по ключу (кэш индекса или встроенные).
    fn members_for(&self, key: &str) -> Vec<Completion> {
        self.members
            .get(key)
            .cloned()
            .unwrap_or_else(|| builtin_members(key))
    }

    /// Тип выражения, кончающегося перед `end` текущего текста: имя, вызов,
    /// цепочка через точку, строка, индекс. Сначала — символ проверки в этом
    /// месте, затем вывод по тексту (для только что набранного).
    fn infer(&self, analyzed: &str, text: &str, end: usize, depth: usize) -> Option<String> {
        if depth > 16 {
            return None;
        }
        let bytes = text.as_bytes();
        let mut end = end;
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }
        if end == 0 {
            return None;
        }
        let scope = scope_offset(analyzed, text, end as u32);
        match bytes[end - 1] {
            b'"' => Some("string".to_string()),
            b']' => {
                let open = matching_open(bytes, end - 1, b'[', b']')?;
                let collection = self.infer(analyzed, text, open, depth + 1)?;
                if let Some(element) = collection.strip_prefix('@') {
                    Some(element.to_string())
                } else if collection.starts_with('%') {
                    collection
                        .split_once(':')
                        .map(|(_, value)| value.to_string())
                } else {
                    None
                }
            }
            b')' => {
                let open = matching_open(bytes, end - 1, b'(', b')')?;
                let mut start = open;
                while start > 0 && is_ident(bytes[start - 1]) {
                    start -= 1;
                }
                if start == open {
                    return None;
                }
                let name = &text[start..open];
                if let Some(found) = self.symbol_type(analyzed, text, start as u32) {
                    return Some(found);
                }
                if start > 0 && bytes[start - 1] == b'.' {
                    let receiver = self.infer(analyzed, text, start - 1, depth + 1)?;
                    return self
                        .members_for(&receiver)
                        .into_iter()
                        .find(|member| {
                            member.label == name && member.kind == CompletionKind::Method
                        })
                        .and_then(|member| result_type(&member.detail));
                }
                if name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
                    return Some(name.to_string());
                }
                self.globals
                    .iter()
                    .find(|global| global.label == name && global.kind == CompletionKind::Function)
                    .and_then(|global| result_type(&global.detail))
            }
            byte if is_ident(byte) => {
                let mut start = end;
                while start > 0 && is_ident(bytes[start - 1]) {
                    start -= 1;
                }
                let name = &text[start..end];
                if start > 0 && bytes[start - 1] == b'.' {
                    return None;
                }
                if name == "self" {
                    return self.scope_object(scope);
                }
                if let Some(found) = self.symbol_type(analyzed, text, start as u32) {
                    return Some(found);
                }
                let mut scopes: Vec<_> = self
                    .scopes
                    .iter()
                    .filter(|candidate| {
                        candidate.span.start <= scope && scope <= candidate.span.end
                    })
                    .collect();
                scopes.sort_by_key(|candidate| candidate.span.len());
                for candidate in &scopes {
                    if let Some((_, ty, _)) = candidate
                        .locals
                        .iter()
                        .rev()
                        .find(|(local, _, _)| local == name)
                    {
                        return Some(ty.clone());
                    }
                }
                let object = self.scope_object(scope)?;
                self.members
                    .get(&object)?
                    .iter()
                    .find(|member| member.kind == CompletionKind::Field && member.label == name)
                    .map(|member| member.detail.clone())
            }
            _ => None,
        }
    }

    /// Тип символа проверки в месте `offset` текущего текста (если оно перенеслось).
    fn symbol_type(&self, analyzed: &str, text: &str, offset: u32) -> Option<String> {
        self.edited_symbol(analyzed, text, offset)?.ty.clone()
    }

    /// Символ проверки в месте `offset` текущего текста. Перенос позиции
    /// неточен у самой правки (`out` вместо `obj` начинается там же), поэтому
    /// имя в этом месте должно совпасть в обоих текстах.
    fn edited_symbol(&self, analyzed: &str, text: &str, offset: u32) -> Option<&super::Symbol> {
        let at = map_offset(analyzed, text, offset)?;
        if name_at(analyzed, at as usize) != name_at(text, offset as usize) {
            return None;
        }
        self.symbols
            .iter()
            .filter(|symbol| symbol.span.start <= at && at < symbol.span.end)
            .min_by_key(|symbol| symbol.span.len())
    }

    /// Дополнения в `offset` текущего текста `text`; инспекция построена по `analyzed`.
    pub fn complete(&self, analyzed: &str, text: &str, offset: u32) -> Vec<Completion> {
        match Context::at(text, offset) {
            Context::Member { anchor, literal } => {
                if literal {
                    return builtin_members("string");
                }
                let name = &text[anchor.0 as usize..anchor.1 as usize];
                // Конец выражения получателя: после `)` вызова или после имени.
                let bytes = text.as_bytes();
                let mut dot = prefix_start(text, offset) as usize;
                while dot > 0 && bytes[dot - 1] == b' ' {
                    dot -= 1;
                }
                let receiver_end = dot.saturating_sub(1);
                if let Some(key) = self.infer(analyzed, text, receiver_end, 0) {
                    let found = self.members_for(&key);
                    if !found.is_empty() {
                        return found;
                    }
                }
                self.members
                    .get(&format!("namespace:{name}"))
                    .or_else(|| self.members.get(&format!("static:{name}")))
                    .cloned()
                    .unwrap_or_default()
            }
            Context::Take { path } => self
                .members
                .get(&format!("module:{path}"))
                .cloned()
                .unwrap_or_default(),
            Context::Plain => {
                let at = scope_offset(analyzed, text, prefix_start(text, offset));
                // В начале аргумента вызова — сначала имена его параметров.
                let mut out = self.argument_names(analyzed, text, offset);
                let mut seen = std::collections::HashSet::new();
                // Ближайшая (самая узкая) область — первой: её имена перекрывают внешние.
                let mut scopes: Vec<_> = self
                    .scopes
                    .iter()
                    .filter(|scope| scope.span.start <= at && at <= scope.span.end)
                    .collect();
                scopes.sort_by_key(|scope| scope.span.len());
                for scope in &scopes {
                    for (name, ty, declared) in &scope.locals {
                        if *declared < at && seen.insert(name.clone()) {
                            out.push(Completion {
                                label: name.clone(),
                                kind: CompletionKind::Variable,
                                detail: ty.clone(),
                                documentation: String::new(),
                                insert: name.clone(),
                                snippet: false,
                            });
                        }
                    }
                }
                if let Some(key) = scopes.iter().find_map(|scope| scope.object.clone()) {
                    for member in self.members.get(&key).into_iter().flatten() {
                        if member.kind == CompletionKind::Field && seen.insert(member.label.clone())
                        {
                            out.push(member.clone());
                        }
                    }
                    out.push(keyword("self"));
                }
                for global in &self.globals {
                    if seen.insert(global.label.clone()) {
                        out.push(global.clone());
                    }
                }
                out.extend(keywords());
                out
            }
        }
    }

    fn scope_object(&self, at: u32) -> Option<String> {
        self.scopes
            .iter()
            .filter(|scope| {
                scope.span.start <= at && at <= scope.span.end && scope.object.is_some()
            })
            .min_by_key(|scope| scope.span.len())
            .and_then(|scope| scope.object.clone())
    }

    /// Подсказка сигнатуры внутри `name(...)`: подпись, параметры и номер
    /// текущего параметра (в именованном вызове — по имени аргумента).
    pub fn signature(
        &self,
        analyzed: &str,
        text: &str,
        offset: u32,
    ) -> Option<(String, Vec<String>, usize)> {
        let site = self.call_site(analyzed, text, offset)?;
        let active = site.active();
        Some((site.label, site.params, active))
    }

    /// Имена ещё не названных параметров (`zip:`) в начале аргумента вызова,
    /// если вызов может быть именованным.
    pub fn argument_names(&self, analyzed: &str, text: &str, offset: u32) -> Vec<Completion> {
        self.call_site(analyzed, text, offset)
            .map(|site| site.argument_names())
            .unwrap_or_default()
    }

    /// Вызов `name(...)`, внутри которого стоит курсор: аргументы до курсора
    /// и подпись — первая из кандидатов, где есть все уже названные имена.
    fn call_site(&self, analyzed: &str, text: &str, offset: u32) -> Option<CallSite> {
        let bytes = text.as_bytes();
        let offset = (offset as usize).min(bytes.len());
        let mut depth = 0usize;
        let mut commas = Vec::new();
        let mut index = offset;
        let open = loop {
            if index == 0 {
                return None;
            }
            index -= 1;
            match bytes[index] {
                b')' | b']' => depth += 1,
                b'(' | b'[' if depth > 0 => depth -= 1,
                b'(' => break index,
                b'[' | b'{' | b'}' | b';' => return None,
                b',' if depth == 0 => commas.push(index),
                _ => {}
            }
        };
        let mut start = open;
        while start > 0 && is_ident(bytes[start - 1]) {
            start -= 1;
        }
        if start == open {
            return None;
        }
        let name = &text[start..open];
        let mut bounds = vec![open + 1];
        bounds.extend(commas.iter().rev().map(|comma| comma + 1));
        let before: Vec<Option<String>> = bounds
            .windows(2)
            .map(|pair| arg_name(&text[pair[0]..pair[1] - 1]).map(str::to_string))
            .collect();
        let current = text[*bounds.last().unwrap_or(&(open + 1))..offset].to_string();
        let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
        let declaration = matches!(
            text[line_start..start].trim(),
            "" | "async" | "private" | "native" | "native async"
        );
        // Встроенные действия (строк, массивов, …) — только позиционные.
        let mut named = !declaration;
        let mut candidates: Vec<String> = self
            .signature_by_symbol(analyzed, text, start as u32)
            .into_iter()
            .collect();
        if start > 0 && bytes[start - 1] == b'.' {
            if let Some(receiver) = self.infer(analyzed, text, start - 1, 0) {
                named &= builtin_members(&receiver).is_empty();
                candidates.extend(
                    self.members_for(&receiver)
                        .into_iter()
                        .filter(|member| member.label == name)
                        .map(|member| match member.detail.strip_prefix("async ") {
                            Some(rest) => format!("async {name}{rest}"),
                            None => format!("{name}{}", member.detail),
                        }),
                );
            }
        } else if name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
            if let Some(ctors) = self.members.get(&format!("ctor:{name}")) {
                let mut ctors: Vec<&Completion> = ctors.iter().collect();
                // Без имён — по числу аргументов: первая форма, где их хватает.
                ctors.sort_by_key(|ctor| parameters(&ctor.detail).len() <= before.len());
                candidates.extend(ctors.into_iter().map(|ctor| ctor.detail.clone()));
            }
        } else {
            candidates.extend(
                self.globals
                    .iter()
                    .filter(|global| {
                        global.label == name && global.kind == CompletionKind::Function
                    })
                    .map(|global| format!("{name}{}", global.detail)),
            );
        }
        let mut wanted: Vec<&str> = before.iter().flatten().map(String::as_str).collect();
        wanted.extend(arg_name(&current));
        let label = candidates
            .iter()
            .find(|label| {
                let params = parameters(label);
                wanted
                    .iter()
                    .all(|name| params.iter().any(|param| param_name(param) == Some(*name)))
            })
            .or_else(|| candidates.first())?
            .clone();
        let params = parameters(&label);
        Some(CallSite {
            label,
            params,
            before,
            current,
            named,
        })
    }

    fn signature_by_symbol(&self, analyzed: &str, text: &str, offset: u32) -> Option<String> {
        let symbol = self.edited_symbol(analyzed, text, offset)?;
        Some(
            symbol
                .hover
                .strip_prefix("```okkama\n")?
                .lines()
                .next()?
                .to_string(),
        )
    }
}

/// Параметры подписи `name(a is int, b is @int) is R`.
fn parameters(label: &str) -> Vec<String> {
    let Some(open) = label.find('(') else {
        return Vec::new();
    };
    let bytes = label.as_bytes();
    let mut depth = 0;
    let mut current = String::new();
    let mut out = Vec::new();
    for (index, ch) in label[open + 1..].char_indices() {
        let byte = bytes[open + 1 + index];
        match byte {
            b'(' | b'<' => depth += 1,
            b')' if depth == 0 => {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                return out;
            }
            b')' | b'>' => depth -= 1,
            b',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    out
}

/// Вызов вокруг курсора: подпись и аргументы, набранные до него.
struct CallSite {
    label: String,
    params: Vec<String>,
    /// Имена аргументов перед текущим (`None` — позиционный).
    before: Vec<Option<String>>,
    /// Текущий аргумент от его начала до курсора.
    current: String,
    /// Вызов может быть именованным: не объявление и не встроенное действие.
    named: bool,
}

impl CallSite {
    /// Имена параметров, если подпись допускает именованный вызов (у
    /// action-значений имён нет, variadic-хвост принимает только позиционные).
    fn names(&self) -> Option<Vec<&str>> {
        if !self.named {
            return None;
        }
        self.params
            .iter()
            .map(|param| {
                let (_, ty) = param.split_once(" is ")?;
                if ty.starts_with("...") {
                    return None;
                }
                param_name(param)
            })
            .collect()
    }

    /// Номер текущего параметра: по имени аргумента, а в именованном вызове
    /// без имени — первый ещё не названный (с набранным началом имени).
    fn active(&self) -> usize {
        let position = self.before.len();
        let Some(names) = self.names() else {
            return position;
        };
        if let Some(name) = arg_name(&self.current) {
            return names
                .iter()
                .position(|param| *param == name)
                .unwrap_or(position);
        }
        if self.before.is_empty() || self.before.iter().any(Option::is_none) {
            return position;
        }
        let typed = self.current.trim();
        let free = |param: &&str| !self.before.iter().flatten().any(|name| name == param);
        names
            .iter()
            .position(|param| free(param) && param.starts_with(typed))
            .or_else(|| names.iter().position(|param| free(param)))
            .unwrap_or(position)
    }

    /// `name:` для параметров, ещё не названных в вызове; в позиционном
    /// вызове (уже есть позиционный аргумент) — ничего: смешивать нельзя.
    fn argument_names(&self) -> Vec<Completion> {
        let Some(names) = self.names() else {
            return Vec::new();
        };
        if self.before.iter().any(Option::is_none) || !self.current.trim().bytes().all(is_ident) {
            return Vec::new();
        }
        names
            .iter()
            .zip(&self.params)
            .filter(|(name, _)| !self.before.iter().flatten().any(|used| used == *name))
            .map(|(name, param)| Completion {
                label: format!("{name}:"),
                kind: CompletionKind::Argument,
                detail: param
                    .split_once(" is ")
                    .map_or(String::new(), |(_, ty)| ty.to_string()),
                documentation: String::new(),
                insert: format!("{name}: "),
                snippet: false,
            })
            .collect()
    }
}

/// Имя параметра в подписи: `zip is int` → `zip`, `:zip is int` → `zip`.
fn param_name(param: &str) -> Option<&str> {
    let (name, _) = param.split_once(" is ")?;
    let name = name.trim().trim_start_matches(':');
    (!name.is_empty() && name.bytes().all(is_ident)).then_some(name)
}

/// Имя именованного аргумента `zip: 12345` (без значения — тоже).
fn arg_name(arg: &str) -> Option<&str> {
    let arg = arg.trim_start();
    let end = arg.bytes().take_while(|byte| is_ident(*byte)).count();
    let (name, rest) = arg.split_at(end);
    let first = name.bytes().next()?;
    let rest = rest.trim_start();
    (!first.is_ascii_digit() && rest.starts_with(':') && !rest.starts_with("::")).then_some(name)
}

fn keyword(word: &str) -> Completion {
    Completion {
        label: word.to_string(),
        kind: CompletionKind::Keyword,
        detail: String::new(),
        documentation: String::new(),
        insert: word.to_string(),
        snippet: false,
    }
}

pub fn keywords() -> Vec<Completion> {
    [
        "let", "if", "else", "match", "case", "for", "loop", "return", "confirm", "throw", "try",
        "catch", "defer", "await", "async", "spawn", "panic", "next", "last", "true", "false",
        "is", "object", "role", "enum", "has", "does", "import", "take", "module", "test",
        "native", "private", "destroy", "cap", "void",
    ]
    .iter()
    .map(|word| keyword(word))
    .collect()
}

fn method(label: &str, params: &[&str], result: &str, doc: &str) -> Completion {
    let snippet = if params.is_empty() {
        format!("{label}()")
    } else {
        let parts: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let name = param.split(" is ").next().unwrap_or(param);
                format!("${{{}:{name}}}", index + 1)
            })
            .collect();
        format!("{label}({})", parts.join(", "))
    };
    Completion {
        label: label.to_string(),
        kind: CompletionKind::Method,
        detail: format!("({}) is {result}", params.join(", ")),
        documentation: doc.to_string(),
        insert: snippet,
        snippet: true,
    }
}

fn functional(name: &str, bindings: &str, initial: bool, doc: &str) -> Completion {
    let insert = if initial {
        format!("{name}(${{1:initial}}) -> {bindings} {{\n\t$0\n}}")
    } else {
        format!("{name}() -> {bindings} {{\n\t$0\n}}")
    };
    Completion {
        label: name.to_string(),
        kind: CompletionKind::Method,
        detail: format!(
            "{name}({}) -> {bindings} {{ … }}",
            if initial { "initial" } else { "" }
        ),
        documentation: doc.to_string(),
        insert,
        snippet: true,
    }
}

/// Встроенные действия значений (§6.6) по ключу типа (`string`, `@T`, `%K:V`, …).
pub fn builtin_members(key: &str) -> Vec<Completion> {
    let conversions = |out: &mut Vec<Completion>, targets: &[&str]| {
        for target in targets {
            out.push(method(target, &[], target, "explicit conversion"));
        }
    };
    let compare = |out: &mut Vec<Completion>| {
        for name in ["less", "less_or_equal", "more", "more_or_equal"] {
            out.push(method(name, &["other is Self"], "bool", ""));
        }
        out.push(method(
            "order",
            &["other is Self"],
            "Order",
            "three-way comparison",
        ));
    };
    let mut out = Vec::new();
    if key.starts_with('@') || key == "[]" {
        out.push(method("size", &[], "int", "number of elements"));
        out.push(method(
            "slice",
            &["from is int", "to is int"],
            key,
            "elements from..to (exclusive)",
        ));
        out.push(method("join", &["separator is string"], "string", ""));
        out.push(method("item", &["index is int"], "T", ""));
        out.push(method("first", &[], "T", ""));
        out.push(method("last", &[], "T", ""));
        out.push(method("cursor", &[], "Cursor<T>", ""));
        out.push(method(
            "ordering",
            &[],
            key,
            "elements sorted by compare.Comparing",
        ));
        out.push(functional(
            "map",
            "${2:item}",
            false,
            "new array of the block's values",
        ));
        out.push(functional(
            "filter",
            "${2:item}",
            false,
            "elements for which the block is true",
        ));
        out.push(functional(
            "reduce",
            "${2:sum}, ${3:item}",
            true,
            "fold from initial",
        ));
    } else if key.starts_with('%') || key == "[:]" {
        out.push(method("size", &[], "int", ""));
        out.push(method("has", &["key is K"], "bool", ""));
        out.push(method("keys", &[], "@K", "keys in key order"));
        out.push(method("values", &[], "@V", "values in key order"));
        out.push(functional("map", "${2:key}, ${3:value}", false, ""));
        out.push(functional("filter", "${2:key}, ${3:value}", false, ""));
        out.push(functional(
            "reduce",
            "${2:sum}, ${3:key}, ${4:value}",
            true,
            "",
        ));
    } else if key == "string" {
        out.push(method("length", &[], "int", "characters"));
        out.push(method("empty", &[], "bool", ""));
        out.push(method("slice", &["from is int", "to is int"], "string", ""));
        out.push(method("split", &["separator is string"], "@string", ""));
        out.push(method(
            "replace",
            &["from is string", "to is string"],
            "string",
            "every occurrence",
        ));
        out.push(method(
            "replace_first",
            &["from is string", "to is string"],
            "string",
            "",
        ));
        out.push(method("trim", &[], "string", ""));
        out.push(method(
            "chomp",
            &[],
            "string",
            "without one trailing LF or CRLF",
        ));
        out.push(method("less", &["other is string"], "bool", ""));
        out.push(method("less_or_equal", &["other is string"], "bool", ""));
        out.push(method("order", &["other is string"], "Order", ""));
        out.push(method("string", &[], "string", ""));
    } else if matches!(key, "int" | "byte") {
        compare(&mut out);
        conversions(&mut out, &["int", "byte", "float", "rat", "string"]);
        if key == "int" {
            out.push(method("abs", &[], "int", ""));
            out.push(method("sign", &[], "int", ""));
            out.push(method("pow", &["exponent is int"], "float", ""));
            out.push(method("sqrt", &[], "float", ""));
        }
        for bits in ["bit_count", "leading_zeros", "trailing_zeros"] {
            out.push(method(bits, &[], "int", ""));
        }
        for bits in ["rotate_left", "rotate_right"] {
            out.push(method(bits, &["count is int"], key, ""));
        }
    } else if key == "float" {
        compare(&mut out);
        conversions(&mut out, &["int", "byte", "float", "rat", "string"]);
        out.push(method("abs", &[], "float", ""));
        out.push(method("sign", &[], "int", ""));
        out.push(method("pow", &["exponent is float"], "float", ""));
        out.push(method("sqrt", &[], "float", ""));
    } else if key == "rat" {
        compare(&mut out);
        conversions(&mut out, &["int", "byte", "float", "rat", "string"]);
        out.push(method("abs", &[], "rat", ""));
        out.push(method("sign", &[], "int", ""));
        out.push(method("numerator", &[], "string", ""));
        out.push(method("denominator", &[], "string", ""));
    } else if key == "bool" {
        out.push(method("string", &[], "string", "\"1\" or \"0\""));
    } else if key == "char" {
        out.push(method("order", &["other is char"], "Order", ""));
    } else if key.starts_with("Future<") {
        out.push(method("cancel", &[], key, "cooperative cancellation"));
        out.push(method("done", &[], "bool", ""));
        out.push(method("cancelled", &[], "bool", ""));
    } else if key == "range" {
        out.push(functional("map", "${2:item}", false, ""));
        out.push(functional("filter", "${2:item}", false, ""));
        out.push(functional("reduce", "${2:sum}, ${3:item}", true, ""));
    }
    out
}
