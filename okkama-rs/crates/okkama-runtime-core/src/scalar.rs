//! Exact scalar conversions shared by the IR interpreter and Sparrow.
use crate::{IntValue, Value, ValueError};

/// Byte shifts discard outgoing bits; both right-shift spellings zero-fill.
pub fn byte_shift(value: u8, count: &IntValue, left: bool) -> Result<u8, &'static str> {
    let count = count
        .to_i64()
        .filter(|count| (0..=7).contains(count))
        .ok_or("Shift count must be in 0..7")? as u32;
    Ok(if left { value << count } else { value >> count })
}

pub fn string(value: &Value) -> Result<String, ValueError> {
    match value {
        Value::Int(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(if *value { "1" } else { "0" }.to_owned()),
        Value::Byte(value) => Ok(value.to_string()),
        _ => Err(ValueError {
            message: "expected int, bool or byte for string conversion".into(),
        }),
    }
}

pub fn byte(value: &IntValue, context: Option<&str>) -> Result<u8, String> {
    value
        .to_i64()
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| {
            let message = format!(
                "Cannot convert int value {} to byte: expected 0..255",
                value
            );
            context.map_or_else(
                || message.clone(),
                |context| format!("{context}: {message}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_byte_shifts_match_masked_unsigned_words() {
        for value in 0..=255u16 {
            for count in 0..=7u32 {
                let exact_count = IntValue::from_i64(i64::from(count));
                assert_eq!(
                    byte_shift(value as u8, &exact_count, true).unwrap(),
                    ((value << count) & 255) as u8
                );
                assert_eq!(
                    byte_shift(value as u8, &exact_count, false).unwrap(),
                    (value >> count) as u8
                );
            }
        }
    }
}
