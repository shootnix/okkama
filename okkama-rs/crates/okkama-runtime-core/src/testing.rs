//! Состояние текущего теста (§14), общее для исполнителя тестов, intrinsics
//! утверждений и natives `runtime/testing` — как `Tests.current` JVM.
use crate::failure::{Failure, Kind};
use std::cell::RefCell;
use std::rc::Rc;
use crate::Value;

/// Исход текущего теста: провалы утверждений и причина `test.skip`.
#[derive(Debug, Default)]
pub struct Case {
    pub failures: Vec<String>,
    pub skipped: Option<String>,
}

thread_local! {
    static CURRENT: RefCell<Option<Case>> = const { RefCell::new(None) };
}

/// Начать тест: провалы утверждений копятся до `finish`.
pub fn begin() {
    CURRENT.with(|current| *current.borrow_mut() = Some(Case::default()));
}

/// Завершить тест: его провалы утверждений и пропуск.
pub fn finish() -> Case {
    CURRENT.with(|current| current.borrow_mut().take().unwrap_or_default())
}

pub fn active() -> bool {
    CURRENT.with(|current| current.borrow().is_some())
}

fn outside() -> Rc<Failure> {
    Failure::new(Kind::Program(
        "The test capability is available only inside a running test".into(),
    ))
}

/// `Tests.fail`: провал утверждения текущего теста.
pub fn fail(message: String) -> Result<(), Rc<Failure>> {
    CURRENT.with(|current| match current.borrow_mut().as_mut() {
        Some(case) => {
            case.failures.push(message);
            Ok(())
        }
        None => Err(outside()),
    })
}

/// `Tests.skip`: тест пропущен по причине `reason` и кончается здесь. Отказ
/// без брошенного значения не ловит ни один `catch`; cleanup идёт как при
/// любом отказе, исполнитель записывает пропуск вместо отказа. Причина —
/// первого `skip`.
pub fn skip(reason: String) -> Result<(), Rc<Failure>> {
    CURRENT.with(|current| match current.borrow_mut().as_mut() {
        Some(case) => {
            case.skipped.get_or_insert(reason);
            Err(Failure::new(Kind::Program("test skipped".into())))
        }
        None => Err(outside()),
    })
}

/// Имена программы для текста значений: case enum и объявление объекта.
pub struct Names<'a> {
    pub case: &'a dyn Fn(usize, usize) -> String,
    pub object: &'a dyn Fn(usize) -> Option<String>,
}

impl std::fmt::Debug for Names<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Names")
    }
}

/// Текст значения в сообщении утверждения: строка — в кавычках, остальное —
/// как `String.valueOf` коллекционного представления JVM.
pub fn show(names: &Names<'_>, value: &Value) -> String {
    match value {
        Value::Str(text) => format!("\"{text}\""),
        Value::Any(inner) => show(names, inner),
        other => java_text(names, other),
    }
}

fn java_text(names: &Names<'_>, value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        // `java.lang.Byte` знаковый.
        Value::Byte(value) => (*value as i8).to_string(),
        // char — `Integer` code point.
        Value::Char(value) => u32::from(*value).to_string(),
        Value::Str(text) => text.to_string(),
        Value::Float(value) => java_double(*value),
        Value::Rat(value) => value.to_string(),
        Value::Enum { declaration, case } => (names.case)(*declaration, *case),
        Value::Any(inner) => java_text(names, inner),
        Value::Array(array) => format!(
            "[{}]",
            array
                .items()
                .iter()
                .map(|item| java_text(names, item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Map(map) => format!(
            "{{{}}}",
            map.entries()
                .iter()
                .map(|(key, item)| format!(
                    "{}={}",
                    java_text(names, &key.to_value()),
                    java_text(names, item)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Undef => "null".into(),
        Value::Object(object) => {
            let name = (names.object)(object.declaration).unwrap_or_else(|| "Cancelled".to_string());
            format!("{name}@{:x}", object.serial)
        }
        other => format!("{other:?}"),
    }
}

/// `Double.toString`: кратчайшие цифры; `d.ddd` при 10^-3 ≤ |x| < 10^7,
/// иначе `d.dddE±n`.
pub fn java_double(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let magnitude = value.abs();
    let scientific = format!("{magnitude:e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("`{:e}` has an exponent");
    let exponent: i32 = exponent.parse().expect("`{:e}` exponent");
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    if (1e-3..1e7).contains(&magnitude) {
        let text = if exponent >= 0 {
            let point = exponent as usize + 1;
            if digits.len() <= point {
                format!("{digits}{}.0", "0".repeat(point - digits.len()))
            } else {
                format!("{}.{}", &digits[..point], &digits[point..])
            }
        } else {
            format!("0.{}{digits}", "0".repeat((-exponent - 1) as usize))
        };
        return format!("{sign}{text}");
    }
    let fraction = if digits.len() > 1 { &digits[1..] } else { "0" };
    format!("{sign}{}.{fraction}E{exponent}", &digits[..1])
}


// ----- test.close, test.text, test.bytes (#187) -----
// The IR engine and Sparrow write these messages; Tests.java on the JVM
// writes the same.

/// `test.close(actual, expected, tolerance)`: within `tolerance` of each
/// other. A NaN is never close; a negative or NaN tolerance is a failure.
pub fn close_failure(
    actual: f64,
    expected: f64,
    tolerance: f64,
    text: impl Fn(f64) -> String,
) -> Option<String> {
    if tolerance.is_nan() || tolerance < 0.0 {
        return Some(format!("tolerance must be zero or more, got {}", text(tolerance)));
    }
    if actual == expected || (actual - expected).abs() <= tolerance {
        return None;
    }
    Some(format!(
        "expected {} within {}, got {} (off by {})",
        text(expected),
        text(tolerance),
        text(actual),
        text((actual - expected).abs())
    ))
}

/// `test.text(actual, expected)`: where the texts part, in scalars and as
/// line and column of the expected text, with eight scalars around it.
pub fn text_failure(actual: &str, expected: &str) -> Option<String> {
    if actual == expected {
        return None;
    }
    let left: Vec<char> = actual.chars().collect();
    let right: Vec<char> = expected.chars().collect();
    let at = left.iter().zip(&right).take_while(|(a, e)| a == e).count();
    let line = 1 + right[..at].iter().filter(|&&c| c == '\n').count();
    let column = 1 + at - right[..at].iter().rposition(|&c| c == '\n').map_or(0, |p| p + 1);
    let start = at.saturating_sub(8);
    let window = |chars: &[char]| -> String {
        quoted(&chars[start.min(chars.len())..(at + 8).min(chars.len())])
    };
    Some(format!(
        "text differs at scalar {at} (line {line}, column {column}): expected {}, got {}; lengths {} and {}",
        window(&right),
        window(&left),
        right.len(),
        left.len()
    ))
}

/// `test.bytes(actual, expected)`: the first offset where they part.
pub fn bytes_failure(actual: &[u8], expected: &[u8]) -> Option<String> {
    if actual == expected {
        return None;
    }
    let at = actual.iter().zip(expected).take_while(|(a, e)| a == e).count();
    if at < actual.len() && at < expected.len() {
        return Some(format!(
            "bytes differ at offset {at}: expected 0x{:02x}, got 0x{:02x}; sizes {} and {}",
            expected[at],
            actual[at],
            expected.len(),
            actual.len()
        ));
    }
    Some(format!(
        "bytes differ in size: expected {}, got {}; equal up to offset {at}",
        expected.len(),
        actual.len()
    ))
}

/// Text in double quotes, with \\, \", \n, \r, \t and \u{…} for other controls.
fn quoted(chars: &[char]) -> String {
    let mut out = String::from("\"");
    for &c in chars {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn messages_name_where_values_part() {
        let text = |value: f64| value.to_string();
        assert_eq!(close_failure(1.0, 1.05, 0.1, text), None);
        assert_eq!(
            close_failure(1.0, 1.5, 0.1, text).unwrap(),
            "expected 1.5 within 0.1, got 1 (off by 0.5)"
        );
        assert!(close_failure(f64::NAN, 1.0, 1.0, text).is_some());
        assert!(close_failure(1.0, 1.0, -1.0, text).unwrap().starts_with("tolerance"));
        assert_eq!(text_failure("abc", "abc"), None);
        assert_eq!(
            text_failure("one\ntwo", "one\ntoo").unwrap(),
            "text differs at scalar 5 (line 2, column 2): expected \"one\\ntoo\", got \"one\\ntwo\"; lengths 7 and 7"
        );
        assert_eq!(
            bytes_failure(&[1, 2, 3], &[1, 9, 3]).unwrap(),
            "bytes differ at offset 1: expected 0x09, got 0x02; sizes 3 and 3"
        );
        assert_eq!(
            bytes_failure(&[1, 2], &[1, 2, 3]).unwrap(),
            "bytes differ in size: expected 3, got 2; equal up to offset 2"
        );
    }
}
