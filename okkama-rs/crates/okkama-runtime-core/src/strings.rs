//! Позиции строк измеряются Unicode code points; диапазон [start, end).
use crate::IntValue;

use crate::integer;

pub fn split<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    if separator.is_empty() {
        text.char_indices()
            .map(|(offset, ch)| &text[offset..offset + ch.len_utf8()])
            .collect()
    } else {
        text.split(separator).collect()
    }
}

/// Unicode White_Space, matching the language's trim contract.
pub fn trim(text: &str) -> &str {
    text.trim_matches(char::is_whitespace)
}

/// D-033: remove exactly one CRLF or LF suffix; a lone CR stays.
pub fn chomp(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

/// Literal, non-overlapping replacement; empty patterns match code point boundaries.
pub fn replace(text: &str, pattern: &str, replacement: &str, first: bool) -> String {
    if first {
        text.replacen(pattern, replacement, 1)
    } else {
        text.replace(pattern, replacement)
    }
}

pub fn slice<'a>(text: &'a str, start: &IntValue, end: &IntValue) -> Result<&'a str, &'static str> {
    let range = integer::slice_range(text.chars().count(), start, end)?;
    let boundary = |index| {
        text.char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .nth(index)
            .expect("validated code point boundary")
    };
    Ok(&text[boundary(range.start)..boundary(range.end)])
}
