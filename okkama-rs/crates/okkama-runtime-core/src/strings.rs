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

pub fn trim_start(text: &str) -> &str {
    text.trim_start_matches(char::is_whitespace)
}

pub fn trim_end(text: &str) -> &str {
    text.trim_end_matches(char::is_whitespace)
}

/// Code point index of the first occurrence of `part` (#210); an empty part
/// is found at 0.
pub fn find(text: &str, part: &str) -> Option<usize> {
    text.find(part).map(|byte| text[..byte].chars().count())
}

/// Code point index of the last occurrence of `part`; an empty part is found
/// at the end.
pub fn find_last(text: &str, part: &str) -> Option<usize> {
    text.rfind(part).map(|byte| text[..byte].chars().count())
}

/// The longest result `repeat` builds, in bytes: past it the program fails
/// instead of exhausting memory.
pub const REPEAT_LIMIT: usize = 1 << 30;

/// `text` `count` times; a negative count or a result over [`REPEAT_LIMIT`]
/// is an error.
pub fn repeat(text: &str, count: &IntValue) -> Result<String, &'static str> {
    let count = integer::word(count)
        .ok()
        .filter(|count| *count >= 0)
        .ok_or("repeat needs a non-negative count")?;
    let size = usize::try_from(count).ok().and_then(|count| text.len().checked_mul(count));
    match size {
        Some(size) if size <= REPEAT_LIMIT => Ok(text.repeat(count as usize)),
        _ => Err("repeat result is too large"),
    }
}

/// A Unicode scalar value from a code point: `char(code)` (#231).
pub fn character(code: &IntValue) -> Result<char, &'static str> {
    integer::word(code)
        .ok()
        .and_then(|code| u32::try_from(code).ok())
        .and_then(char::from_u32)
        .ok_or("char needs a Unicode scalar value (0..0x10FFFF, not a surrogate)")
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

/// Позиции нескольких недавних строк (#52): число code points и байтовое
/// смещение последнего достигнутого индекса. Индекс рядом с ним ищется от
/// него, поэтому `size()` в цикле и срез по одному code point подряд
/// линейны, а не квадратичны. `Weak` не даёт адресу строки достаться другой.
#[derive(Default)]
struct Position {
    text: Option<std::rc::Weak<str>>,
    count: usize,
    char: usize,
    byte: usize,
}

thread_local! {
    static POSITIONS: std::cell::RefCell<([Position; 4], usize)> = std::cell::RefCell::new(Default::default());
}

/// Позиция строки `text`, при необходимости новая (число code points).
fn with_position<R>(text: &std::rc::Rc<str>, f: impl FnOnce(&mut Position) -> R) -> R {
    POSITIONS.with(|positions| {
        let (entries, next) = &mut *positions.borrow_mut();
        let at = match entries.iter().position(|entry| entry.text.as_ref().is_some_and(|weak| std::ptr::eq(weak.as_ptr(), std::rc::Rc::as_ptr(text)))) {
            Some(at) => at,
            None => {
                let at = *next;
                *next = (*next + 1) % entries.len();
                entries[at] = Position { text: Some(std::rc::Rc::downgrade(text)), count: text.chars().count(), char: 0, byte: 0 };
                at
            }
        };
        f(&mut entries[at])
    })
}

/// Число code points строки.
pub fn length(text: &std::rc::Rc<str>) -> usize {
    if text.len() < 64 {
        return text.chars().count();
    }
    with_position(text, |position| position.count)
}

/// Байтовое смещение code point `index` (не больше числа): от ближайшего из
/// начала, конца и последней достигнутой позиции.
fn offset(text: &std::rc::Rc<str>, index: usize) -> usize {
    with_position(text, |position| {
        let (mut char, mut byte) = [(0, 0), (position.count, text.len()), (position.char, position.byte)]
            .into_iter()
            .min_by_key(|(char, _)| char.abs_diff(index))
            .expect("three starts");
        let bytes = text.as_bytes();
        while char < index {
            byte += match bytes[byte] {
                0x00..=0x7F => 1,
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                _ => 4,
            };
            char += 1;
        }
        while char > index {
            byte -= 1;
            while bytes[byte] & 0xC0 == 0x80 {
                byte -= 1;
            }
            char -= 1;
        }
        (position.char, position.byte) = (char, byte);
        byte
    })
}

pub fn slice<'a>(text: &'a std::rc::Rc<str>, start: &IntValue, end: &IntValue) -> Result<&'a str, &'static str> {
    if text.len() < 64 {
        // Short strings: counting is cheaper than the cache.
        let range = integer::slice_range(text.chars().count(), start, end)?;
        let boundary = |index| {
            text.char_indices().map(|(offset, _)| offset).chain(std::iter::once(text.len())).nth(index).expect("validated code point boundary")
        };
        return Ok(&text[boundary(range.start)..boundary(range.end)]);
    }
    let range = integer::slice_range(length(text), start, end)?;
    let (first, last) = (offset(text, range.start), offset(text, range.end));
    Ok(&text[first..last])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slices agree with a plain count from the start, sequentially, backwards
    /// and at random, on long strings that use the position cache (#52).
    #[test]
    fn slices_match_a_plain_count() {
        let text: std::rc::Rc<str> = "жёлтый 😀 e\u{301} 中文 ok · ".repeat(20).into();
        let count = text.chars().count();
        let plain = |a: usize, b: usize| text.chars().skip(a).take(b - a).collect::<String>();
        let int = |n: usize| IntValue::from_i64(n as i64);
        assert_eq!(length(&text), count);
        for index in (0..count).chain((0..count).rev()) {
            assert_eq!(slice(&text, &int(index), &int(index + 1)).unwrap(), plain(index, index + 1));
        }
        let mut state: u64 = 11;
        for _ in 0..1000 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let a = (state >> 33) as usize % (count + 1);
            let b = a + (state >> 17) as usize % (count + 1 - a);
            assert_eq!(slice(&text, &int(a), &int(b)).unwrap(), plain(a, b));
        }
    }
}
