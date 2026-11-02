//! Поведение библиотеки Java, на которое опираются providers JVM: регулярные
//! выражения `java.util.regex` (через `fancy-regex`), `String.split`,
//! `replaceAll`/`replaceFirst` с буквальной заменой, строгие кодировки
//! `Charset` (REPORT).
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
        cache.borrow_mut().insert(pattern.to_string(), compiled.clone());
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
    regex(pattern).replace_all(text, fancy_regex::NoExpand(replacement)).into_owned()
}

/// `replaceFirst(regex, quoteReplacement(replacement))`.
pub fn replace_first(text: &str, pattern: &str, replacement: &str) -> String {
    regex(pattern).replace(text, fancy_regex::NoExpand(replacement)).into_owned()
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

/// Строгое декодирование `Charset` (REPORT): None — некорректные байты.
pub fn decode(bytes: &[u8], encoding: &str) -> Option<String> {
    match encoding {
        "utf-8" => String::from_utf8(bytes.to_vec()).ok(),
        "ascii" => bytes.iter().all(u8::is_ascii).then(|| bytes.iter().map(|b| char::from(*b)).collect()),
        "latin-1" => Some(bytes.iter().map(|b| char::from(*b)).collect()),
        "utf-16-le" | "utf-16-be" => {
            if bytes.len() % 2 != 0 {
                return None;
            }
            let units: Vec<u16> = bytes
                .chunks(2)
                .map(|pair| {
                    if encoding == "utf-16-le" {
                        u16::from_le_bytes([pair[0], pair[1]])
                    } else {
                        u16::from_be_bytes([pair[0], pair[1]])
                    }
                })
                .collect();
            String::from_utf16(&units).ok()
        }
        "utf-32-le" | "utf-32-be" => {
            if bytes.len() % 4 != 0 {
                return None;
            }
            bytes
                .chunks(4)
                .map(|quad| {
                    let quad: [u8; 4] = quad.try_into().expect("four bytes");
                    let value = if encoding == "utf-32-le" {
                        u32::from_le_bytes(quad)
                    } else {
                        u32::from_be_bytes(quad)
                    };
                    char::from_u32(value)
                })
                .collect()
        }
        _ => None,
    }
}

/// Строгое кодирование `Charset` (REPORT): None — непредставимый символ.
pub fn encode(text: &str, encoding: &str) -> Option<Vec<u8>> {
    match encoding {
        "utf-8" => Some(text.as_bytes().to_vec()),
        "ascii" => text.is_ascii().then(|| text.as_bytes().to_vec()),
        "latin-1" => text.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect(),
        "utf-16-le" => Some(text.encode_utf16().flat_map(u16::to_le_bytes).collect()),
        "utf-16-be" => Some(text.encode_utf16().flat_map(u16::to_be_bytes).collect()),
        "utf-32-le" => Some(text.chars().flat_map(|c| u32::from(c).to_le_bytes()).collect()),
        "utf-32-be" => Some(text.chars().flat_map(|c| u32::from(c).to_be_bytes()).collect()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn split_follows_java_trailing_rules() {
        assert_eq!(split("a/b//", "/", 0), vec!["a", "b"]);
        assert_eq!(split("a/b//", "/", -1), vec!["a", "b", "", ""]);
        assert_eq!(split("/a", "/", 0), vec!["", "a"]);
        assert_eq!(split("", "/", 0), vec![""]);
        assert_eq!(split("abc", "/", 0), vec!["abc"]);
        assert_eq!(split("x,y,z", ",", 2), vec!["x", "y,z"]);
    }
}

/// `Double.parseDouble` (без обрамляющих пробелов): знак, `NaN`,
/// `Infinity`, десятичная или шестнадцатеричная запись, суффикс `fFdD`.
pub fn parse_double(text: &str) -> Option<f64> {
    let (negative, body) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let signed = |value: f64| if negative { -value } else { value };
    match body {
        "NaN" => return Some(f64::NAN),
        "Infinity" => return Some(signed(f64::INFINITY)),
        _ => {}
    }
    let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
    if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        let (mantissa, exponent) = hex.split_once(['p', 'P'])?;
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if whole.is_empty() && fraction.is_empty() {
            return None;
        }
        let digits = format!("{whole}{fraction}");
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let exponent: i64 = exponent.parse().ok()?;
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            return Some(signed(0.0));
        }
        // Значащие биты с липким битом для корректного округления.
        let mut value: u128 = 0;
        let mut shift = exponent - 4 * fraction.len() as i64;
        let mut sticky = false;
        for (index, digit) in digits.bytes().enumerate() {
            let nibble = u128::from((digit as char).to_digit(16).expect("hex"));
            if index < 30 {
                value = (value << 4) | nibble;
            } else {
                shift += 4;
                sticky |= nibble != 0;
            }
        }
        let value = (value << 1) | u128::from(sticky);
        shift -= 1;
        return Some(signed(scaled(value, shift)));
    }
    let valid = !body.is_empty()
        && body.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
        && body.bytes().any(|b| b.is_ascii_digit());
    if !valid {
        return None;
    }
    body.parse::<f64>().ok().map(signed)
}

/// `value × 2^shift`, округлённое к ближайшему чётному binary64.
fn scaled(value: u128, shift: i64) -> f64 {
    if value == 0 {
        return 0.0;
    }
    let bits = 128 - i64::from(value.leading_zeros());
    let exponent = bits - 1 + shift;
    // Нормальные числа: 53 бита; субнормальные — меньше.
    let precision = if exponent < -1022 { 53 - (-1022 - exponent) } else { 53 };
    if precision <= 0 {
        let half = precision == 0 && value > (1u128 << (bits - 1));
        return if half { f64::from_bits(1) } else { 0.0 };
    }
    let drop = bits - precision;
    let mut mantissa = if drop > 0 { value >> drop } else { value << (-drop) };
    if drop > 0 {
        let rest = value & ((1u128 << drop) - 1);
        let half = 1u128 << (drop - 1);
        if rest > half || (rest == half && mantissa & 1 == 1) {
            mantissa += 1;
        }
    }
    let shift = shift + drop.max(0) - (-drop).max(0);
    (mantissa as f64) * 2f64.powi(shift as i32)
}
