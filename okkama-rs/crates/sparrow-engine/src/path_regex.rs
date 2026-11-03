//! Pure fixed-pattern compatibility helpers for portable lexical paths.
use fancy_regex::Regex;
use std::collections::HashMap;

/// Скомпилированное выражение (кэш на поток).
pub fn regex(pattern: &str) -> std::rc::Rc<Regex> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<String, std::rc::Rc<Regex>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    CACHE.with(|cache| {
        if let Some(found) = cache.borrow().get(pattern) {
            return found.clone();
        }
        let compiled = std::rc::Rc::new(Regex::new(pattern).expect("provider regex"));
        cache
            .borrow_mut()
            .insert(pattern.to_string(), compiled.clone());
        compiled
    })
}

pub fn is_match(pattern: &str, text: &str) -> bool {
    regex(pattern).is_match(text).unwrap_or(false)
}

/// `String.matches`: выражение покрывает всю строку.
pub fn matches(pattern: &str, text: &str) -> bool {
    is_match(&format!(r"\A(?:{pattern})\z"), text)
}

/// `replaceAll(regex, quoteReplacement(replacement))`.
pub fn replace_all(text: &str, pattern: &str, replacement: &str) -> String {
    regex(pattern)
        .replace_all(text, fancy_regex::NoExpand(replacement))
        .into_owned()
}

/// `replaceFirst(regex, quoteReplacement(replacement))`.
pub fn replace_first(text: &str, pattern: &str, replacement: &str) -> String {
    regex(pattern)
        .replace(text, fancy_regex::NoExpand(replacement))
        .into_owned()
}

/// Группы первого совпадения (`find`), None — нет совпадения.
pub fn captures(pattern: &str, text: &str) -> Option<Vec<Option<String>>> {
    let found = regex(pattern).captures(text).ok().flatten()?;
    Some(
        (0..found.len())
            .map(|index| found.get(index).map(|group| group.as_str().to_string()))
            .collect(),
    )
}

/// Начало и конец первого совпадения в байтах.
pub fn find(pattern: &str, text: &str) -> Option<(usize, usize)> {
    regex(pattern)
        .find(text)
        .ok()
        .flatten()
        .map(|found| (found.start(), found.end()))
}

/// `String.split(regex, limit)` Java: `limit == 0` отбрасывает хвостовые
/// пустые поля; совпадение нулевой ширины в начале не даёт пустого поля.
pub fn split(text: &str, pattern: &str, limit: i32) -> Vec<String> {
    let compiled = regex(pattern);
    let mut out = Vec::new();
    let mut last = 0;
    let mut position = 0;
    while position <= text.len() {
        if limit > 0 && out.len() as i32 == limit - 1 {
            break;
        }
        let Ok(Some(found)) = compiled.find_from_pos(text, position) else {
            break;
        };
        if found.end() == 0 && found.start() == 0 {
            position = next_boundary(text, 0);
            continue;
        }
        if found.start() == found.end() && found.start() >= text.len() {
            break;
        }
        out.push(text[last..found.start()].to_string());
        last = found.end();
        position = if found.start() == found.end() {
            next_boundary(text, found.end())
        } else {
            found.end()
        };
    }
    if out.is_empty() && last == 0 {
        return vec![text.to_string()];
    }
    out.push(text[last..].to_string());
    if limit == 0 {
        while out.len() > 1 && out.last().is_some_and(String::is_empty) {
            out.pop();
        }
        if out.len() == 1 && out[0].is_empty() && !text.is_empty() {
            out.pop();
        }
    }
    out
}

fn next_boundary(text: &str, at: usize) -> usize {
    let mut next = at + 1;
    while next < text.len() && !text.is_char_boundary(next) {
        next += 1;
    }
    next
}
