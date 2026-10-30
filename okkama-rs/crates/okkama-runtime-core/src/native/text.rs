//! Natives байтовых и текстовых преобразований — перенос части `TextNatives`
//! JVM: `runtime/binary`, `runtime/hpack`, `runtime/collections`,
//! `runtime/number_text` (`runtime/buffer` и `runtime/single_byte` — в своих
//! модулях).
use super::Error;
use super::support::{
    byte_array, bytes, failed, failure, float, int, items, string, string_map,
};
use crate::bignum::Big;
use crate::{IntValue, Value, abi::Native};

#[path = "hpack_table.rs"]
mod hpack_table;

fn operation(native: Native) -> &'static str {
    native
        .symbol()
        .split("::")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
}

// ---- runtime/binary ----

fn integer_fits(native: Native, value: &IntValue, size: &IntValue, signedness: &str) -> Result<bool, Error> {
    if size.signum() <= 0 {
        return Ok(false);
    }
    let size = size
        .to_i64()
        .filter(|size| *size <= 1 << 20)
        .ok_or_else(|| failed(native))? as u64;
    let bits = 8 * size;
    let magnitude = value.to_big().abs();
    match signedness {
        "unsigned" => Ok(!value.is_negative() && magnitude.bits() <= bits),
        "signed" => {
            if !value.is_negative() {
                return Ok(magnitude.bits() < bits);
            }
            // -2^(n-1) ≤ v: |v| < 2^(n-1) или |v| = 2^(n-1).
            let length = magnitude.bits();
            Ok(length < bits
                || (length == bits && magnitude.subtract(&Big::from_u64(1)).bits() == bits - 1))
        }
        _ => Err(failed(native)),
    }
}

/// Байты дополнительного кода `value` ширины `width` в порядке старший→младший.
fn twos_complement(value: &IntValue, width: usize) -> Vec<u8> {
    let mut out = vec![0u8; width];
    let modulus = Big::from_u64(256);
    let mut number = value.to_big();
    if number.sign() < 0 {
        number = number.add(&Big::from_u64(2).power(8 * width as u32));
    }
    for index in (0..width).rev() {
        if number.is_zero() {
            break;
        }
        let (quotient, remainder) = number.divide(&modulus);
        out[index] = remainder.to_i64().expect("byte") as u8;
        number = quotient;
    }
    out
}

fn ordered(native: Native, origin: &Value, order: &str) -> Result<Vec<u8>, Error> {
    let mut bytes = bytes(origin)?;
    match order {
        "big" => {}
        "little" => bytes.reverse(),
        _ => return Err(failed(native)),
    }
    Ok(bytes)
}

fn integer_from_bytes(native: Native, big: &[u8], signedness: &str) -> Result<IntValue, Error> {
    if big.is_empty() {
        return Err(failed(native));
    }
    let mut number = Big::zero();
    let base = Big::from_u64(256);
    for byte in big {
        number = number.multiply(&base).add(&Big::from_u64(u64::from(*byte)));
    }
    match signedness {
        "signed" => {
            if big[0] & 0x80 != 0 {
                number = number.subtract(&Big::from_u64(2).power(8 * big.len() as u32));
            }
        }
        "unsigned" => {}
        _ => return Err(failed(native)),
    }
    Ok(IntValue::from_big(number))
}

fn float_width(native: Native, width: &IntValue) -> Result<u32, Error> {
    match width.to_i64() {
        Some(32) => Ok(32),
        Some(64) => Ok(64),
        _ => Err(failed(native)),
    }
}

fn float_bytes(value: f64, width: u32) -> Vec<u8> {
    if width == 32 {
        (value as f32).to_bits().to_be_bytes().to_vec()
    } else {
        value.to_bits().to_be_bytes().to_vec()
    }
}

fn float_finite(big: &[u8], width: u32) -> bool {
    if width == 32 {
        big.len() == 4 && ((((big[0] & 0x7f) as u32) << 1) | ((big[1] as u32) >> 7)) != 0xff
    } else {
        big.len() == 8 && ((((big[0] & 0x7f) as u32) << 4) | ((big[1] as u32) >> 4)) != 0x7ff
    }
}

// ---- runtime/hpack ----

fn huffman_decode(native: Native, origin: &[u8], capacity: &IntValue) -> Result<Vec<u8>, Error> {
    let maximum = capacity
        .to_i64()
        .filter(|capacity| (0..=1_048_576).contains(capacity))
        .filter(|_| origin.len() <= 1_048_576)
        .ok_or_else(|| failure("core.hpack", "invalid_input", native, "invalid HPACK Huffman bounds"))?
        as usize;
    let nodes = &hpack_table::HUFFMAN;
    let mut node: usize = 0;
    let mut out = Vec::new();
    for octet in origin {
        for shift in (0..8).rev() {
            let bit = usize::from((octet >> shift) & 1);
            let next = nodes[node * 4 + bit];
            if next < 0 {
                return Err(failure("core.hpack", "invalid_data", native, "invalid HPACK Huffman code"));
            }
            node = next as usize;
            let decoded = nodes[node * 4 + 2];
            if decoded >= 0 {
                if decoded == 256 {
                    return Err(failure(
                        "core.hpack",
                        "invalid_data",
                        native,
                        "HPACK Huffman EOS is forbidden",
                    ));
                }
                if out.len() >= maximum {
                    return Err(failure(
                        "core.hpack",
                        "resource_exhausted",
                        native,
                        "decoded HPACK Huffman limit",
                    ));
                }
                out.push(decoded as u8);
                node = 0;
            }
        }
    }
    if nodes[node * 4 + 3] == 0 {
        return Err(failure("core.hpack", "invalid_data", native, "invalid HPACK Huffman padding"));
    }
    Ok(out)
}

// ---- runtime/collections ----

fn star_match(native: Native, pattern: &str, target: &str, anchored: bool, limit: &IntValue) -> Result<bool, Error> {
    let budget = limit
        .to_i64()
        .filter(|limit| (1..=262_144).contains(limit))
        .ok_or_else(|| {
            failure(
                "core.collections",
                "invalid_input",
                native,
                "Invalid string star-match step budget",
            )
        })?;
    let left: Vec<char> = pattern.chars().collect();
    let right: Vec<char> = target.chars().collect();
    let (mut position, mut at, mut star, mut retry, mut steps) = (0usize, 0usize, None, 0usize, 0i64);
    loop {
        steps += 1;
        if steps > budget {
            return Err(failure(
                "core.collections",
                "resource_exhausted",
                native,
                "String star-match step limit",
            ));
        }
        if position == left.len() && (!anchored || at == right.len()) {
            return Ok(true);
        }
        if position < left.len() && left[position] == '*' {
            star = Some(position);
            retry = at;
            position += 1;
            continue;
        }
        if at < right.len() && position < left.len() && left[position] == right[at] {
            at += 1;
            position += 1;
            continue;
        }
        if let Some(star) = star {
            if retry < right.len() {
                retry += 1;
                at = retry;
                position = star + 1;
                continue;
            }
        }
        return Ok(false);
    }
}

fn map_snapshot(native: Native, args: &[Value]) -> Result<Value, Error> {
    let invalid = |code: &str, message: &str| failure("core.collections", code, native, message);
    let entries = int(&args[2])?
        .to_i64()
        .filter(|entries| (0..=4096).contains(entries));
    let characters = int(&args[3])?
        .to_i64()
        .filter(|characters| (0..=1_048_576).contains(characters));
    let (Some(entries), Some(maximum)) = (entries, characters) else {
        return Err(invalid("invalid_input", "Invalid string map snapshot budgets"));
    };
    let (keys, values) = (items(&args[0])?, items(&args[1])?);
    if keys.len() != values.len() {
        return Err(invalid(
            "invalid_input",
            "String map coordinates have different lengths",
        ));
    }
    if keys.len() as i64 > entries {
        return Err(invalid("resource_exhausted", "String map entry limit"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut used: i64 = 0;
    let mut out = Vec::new();
    for (key, value) in keys.iter().zip(values) {
        let (key, value) = (key.as_string()?, value.as_string()?);
        if !seen.insert(key) {
            return Err(invalid("invalid_input", "String map has duplicate keys"));
        }
        let width = (key.chars().count() + value.chars().count()) as i64;
        if width > maximum - used {
            return Err(invalid("resource_exhausted", "String map character limit"));
        }
        used += width;
        out.push((key.to_string(), value.to_string()));
    }
    Ok(string_map(out))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    match operation(native) {
        "binary_masked_slice" => {
            let origin = bytes(&args[0])?;
            let mask = bytes(&args[1])?;
            let length = origin.len() as i64;
            let (phase, begin, count) = (int(&args[2])?, int(&args[3])?, int(&args[4])?);
            let phase = phase.to_i64().filter(|phase| (0..=3).contains(phase));
            let begin = begin.to_i64().filter(|begin| (0..=length).contains(begin));
            let count = count.to_i64().filter(|count| (0..=65_536).contains(count));
            let (Some(phase), Some(begin), Some(count)) = (phase, begin, count) else {
                return Err(failure(
                    "core.binary",
                    "invalid_input",
                    native,
                    "invalid bounded byte mask coordinates",
                ));
            };
            if mask.len() != 4 || count > length - begin {
                return Err(failure(
                    "core.binary",
                    "invalid_input",
                    native,
                    "invalid bounded byte mask coordinates",
                ));
            }
            let rotation = ((phase + begin) % 4) as usize;
            let start = begin as usize;
            Ok(byte_array(
                (0..count as usize).map(|index| origin[start + index] ^ mask[(rotation + index) % 4]),
            ))
        }
        "binary_integer_fits" => Ok(Value::Bool(integer_fits(
            native,
            int(&args[0])?,
            int(&args[1])?,
            string(&args[2])?,
        )?)),
        "binary_integer_bytes" => {
            let (value, size) = (int(&args[0])?, int(&args[1])?);
            if !integer_fits(native, value, size, string(&args[2])?)? {
                return Err(failed(native));
            }
            let width = size.to_i64().expect("fits") as usize;
            let mut out = twos_complement(value, width);
            match string(&args[3])? {
                "big" => {}
                "little" => out.reverse(),
                _ => return Err(failed(native)),
            }
            Ok(byte_array(out))
        }
        "binary_integer_from_bytes_fits" => {
            let big = ordered(native, &args[0], string(&args[2])?)?;
            Ok(Value::Bool(
                !big.is_empty()
                    && integer_from_bytes(native, &big, string(&args[1])?)?
                        .to_i64()
                        .is_some(),
            ))
        }
        "binary_integer_from_bytes" => {
            let big = ordered(native, &args[0], string(&args[2])?)?;
            let number = integer_from_bytes(native, &big, string(&args[1])?)?;
            if number.to_i64().is_none() {
                return Err(failed(native));
            }
            Ok(Value::Int(number))
        }
        "binary_float_fits" => {
            let width = float_width(native, int(&args[1])?)?;
            Ok(Value::Bool(float_finite(
                &float_bytes(float(&args[0])?, width),
                width,
            )))
        }
        "binary_float_bytes" => {
            let width = float_width(native, int(&args[1])?)?;
            let mut big = float_bytes(float(&args[0])?, width);
            match string(&args[2])? {
                "big" => {}
                "little" => big.reverse(),
                _ => return Err(failed(native)),
            }
            Ok(byte_array(big))
        }
        "binary_float_finite" => {
            let width = float_width(native, int(&args[1])?)?;
            let big = ordered(native, &args[0], string(&args[2])?)?;
            Ok(Value::Bool(float_finite(&big, width)))
        }
        "binary_float_from_bytes" => {
            let width = float_width(native, int(&args[1])?)?;
            let big = ordered(native, &args[0], string(&args[2])?)?;
            if !float_finite(&big, width) {
                return Err(failed(native));
            }
            let raw = big.iter().fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte));
            Ok(Value::Float(if width == 32 {
                f64::from(f32::from_bits(raw as u32))
            } else {
                f64::from_bits(raw)
            }))
        }
        "huffman_decode" => Ok(byte_array(huffman_decode(
            native,
            &bytes(&args[0])?,
            int(&args[1])?,
        )?)),
        "string_star_match" => Ok(Value::Bool(star_match(
            native,
            string(&args[0])?,
            string(&args[1])?,
            args[2].as_bool()?,
            int(&args[3])?,
        )?)),
        "string_map_snapshot" => map_snapshot(native, args),
        "int_parse_valid" => Ok(Value::Bool(int_parse(string(&args[0])?, int(&args[1])?).is_some())),
        "int_parse" => int_parse(string(&args[0])?, int(&args[1])?)
            .map(Value::Int)
            .ok_or_else(|| failed(native)),
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}

// ---- runtime/number_text ----

/// Целое в записи `strings/parse::ParsedInt`: необязательный `-`, при
/// `radix == 0` — префикс `0b`/`0o`/`0x` (иначе основание 10), затем хотя бы
/// одна цифра `0-9a-zA-Z` меньше основания. Длина записи не ограничена.
fn int_parse(text: &str, radix: &IntValue) -> Option<IntValue> {
    let radix = radix.to_i64()?;
    if radix != 0 && !(2..=36).contains(&radix) {
        return None;
    }
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (base, digits) = if radix == 0 {
        match unsigned.get(..2) {
            Some("0b" | "0B") => (2, &unsigned[2..]),
            Some("0o" | "0O") => (8, &unsigned[2..]),
            Some("0x" | "0X") => (16, &unsigned[2..]),
            _ => (10, unsigned),
        }
    } else {
        (radix as u32, unsigned)
    };
    if digits.is_empty() {
        return None;
    }
    let mut small: u64 = 0;
    let mut big: Option<IntValue> = None;
    for byte in digits.bytes() {
        let digit = (byte as char).to_digit(36).filter(|digit| *digit < base)?;
        match &mut big {
            None => match small.checked_mul(u64::from(base)).and_then(|value| value.checked_add(u64::from(digit))) {
                Some(value) => small = value,
                None => {
                    let value = IntValue::from_u128(u128::from(small));
                    big = Some(crate::integer::add(
                        &crate::integer::multiply(&value, &IntValue::from_i64(i64::from(base))),
                        &IntValue::from_i64(i64::from(digit)),
                    ));
                }
            },
            Some(value) => {
                *value = crate::integer::add(
                    &crate::integer::multiply(value, &IntValue::from_i64(i64::from(base))),
                    &IntValue::from_i64(i64::from(digit)),
                );
            }
        }
    }
    let magnitude = big.unwrap_or_else(|| IntValue::from_u128(u128::from(small)));
    Some(if negative { magnitude.negated() } else { magnitude })
}

#[cfg(test)]
mod number_text_tests {
    use super::*;

    fn parsed(text: &str, radix: i64) -> Option<String> {
        int_parse(text, &IntValue::from_i64(radix)).map(|value| value.to_string())
    }

    #[test]
    fn integers_parse_like_parsed_int() {
        assert_eq!(parsed("0", 10).as_deref(), Some("0"));
        assert_eq!(parsed("-42", 10).as_deref(), Some("-42"));
        assert_eq!(parsed("0042", 10).as_deref(), Some("42"));
        assert_eq!(parsed("ff", 16).as_deref(), Some("255"));
        assert_eq!(parsed("Z", 36).as_deref(), Some("35"));
        assert_eq!(parsed("0b101010", 0).as_deref(), Some("42"));
        assert_eq!(parsed("-0X2A", 0).as_deref(), Some("-42"));
        assert_eq!(parsed("0x", 0), None);
        assert_eq!(parsed("0x1f", 16), None);
        assert_eq!(
            parsed("123456789012345678901234567890", 10).as_deref(),
            Some("123456789012345678901234567890")
        );
        assert_eq!(parsed("-18446744073709551616", 10).as_deref(), Some("-18446744073709551616"));
        for bad in ["", "-", "+1", "1.0", " 1", "1 ", "12x", "--1", "é"] {
            assert_eq!(parsed(bad, 10), None, "{bad}");
        }
        assert_eq!(parsed("1", 37), None);
        assert_eq!(parsed("1", 1), None);
    }
}
