//! Состояние текущего теста (§14), общее для исполнителя тестов, intrinsics
//! утверждений и natives `runtime/testing` — как `Tests.current` JVM.
use crate::failure::{Failure, Kind};
use std::cell::RefCell;
use std::rc::Rc;
use crate::Value;

thread_local! {
    static CURRENT: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Начать тест: провалы утверждений копятся до `finish`.
pub fn begin() {
    CURRENT.with(|current| *current.borrow_mut() = Some(Vec::new()));
}

/// Завершить тест: его провалы утверждений.
pub fn finish() -> Vec<String> {
    CURRENT.with(|current| current.borrow_mut().take().unwrap_or_default())
}

pub fn active() -> bool {
    CURRENT.with(|current| current.borrow().is_some())
}

/// `Tests.fail`: провал утверждения текущего теста.
pub fn fail(message: String) -> Result<(), Rc<Failure>> {
    CURRENT.with(|current| match current.borrow_mut().as_mut() {
        Some(failures) => {
            failures.push(message);
            Ok(())
        }
        None => Err(Failure::new(Kind::Program(
            "The test capability is available only inside a running test".into(),
        ))),
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
                .0
                .iter()
                .map(|item| java_text(names, item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Map(map) => format!(
            "{{{}}}",
            map.0
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

