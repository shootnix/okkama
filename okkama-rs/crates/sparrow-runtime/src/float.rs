//! `float` (OIR-1 §16.2): конечное binary64. Неконечный результат и делитель 0 —
//! ошибки программы с текстами JVM-рантайма (`okkama.rt.Floats`).

use crate::IntValue;
use std::cmp::Ordering;

pub type Result<T> = std::result::Result<T, String>;

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Float value must contain a finite number".into())
    }
}

pub fn add(left: f64, right: f64) -> Result<f64> {
    finite(left + right)
}

pub fn sub(left: f64, right: f64) -> Result<f64> {
    finite(left - right)
}

pub fn mul(left: f64, right: f64) -> Result<f64> {
    finite(left * right)
}

pub fn div(left: f64, right: f64) -> Result<f64> {
    if right == 0.0 {
        return Err("Division by zero".into());
    }
    finite(left / right)
}

/// Остаток со знаком делимого (`fmod`, как `%` Java и `math.Mod` Go).
pub fn modulo(left: f64, right: f64) -> Result<f64> {
    if right == 0.0 {
        return Err("Modulo by zero".into());
    }
    finite(left % right)
}

pub fn pow(base: f64, exponent: f64) -> Result<f64> {
    finite(base.powf(exponent))
}

pub fn neg(value: f64) -> f64 {
    -value
}

pub fn abs(value: f64) -> f64 {
    value.abs()
}

pub fn sqrt(value: f64) -> Result<f64> {
    if value < 0.0 {
        return Err(format!("Can't take sqrt of {}", text(value)));
    }
    Ok(value.sqrt())
}

pub fn sign(value: f64) -> i64 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// IEEE-сравнение: `-0.0 == 0.0`; NaN в языке не бывает.
pub fn compare(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

pub fn from_int(value: &IntValue) -> Result<f64> {
    finite(value.to_f64())
}

/// Усечение к нулю; результат — только signed-64.
pub fn to_int(value: f64) -> Result<IntValue> {
    let truncated = value.trunc();
    // [-2^63, 2^63) точно представимы границами binary64.
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&truncated) {
        return Err(format!(
            "Cannot convert float value {} to int: expected signed-64 range",
            text(value)
        ));
    }
    Ok(IntValue::from_i64(truncated as i64))
}

pub fn to_byte(value: f64, context: Option<&str>) -> Result<u8> {
    let truncated = value.trunc();
    if !(0.0..=255.0).contains(&truncated) {
        let message = format!(
            "Cannot convert float value {} to byte: expected 0..255",
            text(value)
        );
        return Err(match context {
            Some(context) => format!("{context}: {message}"),
            None => message,
        });
    }
    Ok(truncated as u8)
}

/// Текст по правилу Go `strconv.FormatFloat(x, 'g', -1, 64)` (D-032):
/// кратчайшие цифры, экспонента при порядке < -4 или ≥ 6 (`1e+06`, `2.5e-07`),
/// знак нуля сохраняется (`-0`).
pub fn text(value: f64) -> String {
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value == 0.0 {
        return format!("{sign}0");
    }
    // `{:e}` даёт кратчайшие цифры: `1.234567e6`, `3e-1`.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').expect("`{:e}` has an exponent");
    let exponent: i32 = exponent.parse().expect("`{:e}` exponent is an integer");
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    if !(-4..6).contains(&exponent) {
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        return format!(
            "{sign}{first}{fraction}e{exponent_sign}{:02}",
            exponent.abs()
        );
    }
    if exponent < 0 {
        let zeros = "0".repeat((-exponent - 1) as usize);
        return format!("{sign}0.{zeros}{digits}");
    }
    let point = exponent as usize + 1;
    if digits.len() <= point {
        format!("{sign}{digits}{}", "0".repeat(point - digits.len()))
    } else {
        format!("{sign}{}.{}", &digits[..point], &digits[point..])
    }
}

/// Точная десятичная запись конечного binary64: цифры модуля без ведущих
/// нулей и показатель — значение равно `digits × 10^exponent`.
pub fn exact_decimal(value: f64) -> (String, i64) {
    if value == 0.0 {
        return ("0".into(), 0);
    }
    let bits = value.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1 << 52) - 1);
    let (mantissa, power) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    let mantissa = crate::bignum::Big::from_u64(mantissa);
    if power >= 0 {
        let scaled = mantissa.multiply(&crate::bignum::Big::from_u64(2).power(power as u32));
        return (scaled.to_string(), 0);
    }
    // m × 2^-k = m × 5^k / 10^k.
    let scaled = mantissa.multiply(&crate::bignum::Big::from_u64(5).power((-power) as u32));
    (scaled.to_string(), power)
}

/// Округление цифр `digits × 10^exponent` до `precision` значащих (half even):
/// цифры длины не больше `precision` и новый показатель.
pub fn round_significant(digits: &str, exponent: i64, precision: usize) -> (String, i64) {
    if digits.len() <= precision {
        return (digits.to_string(), exponent);
    }
    let cut = digits.len() - precision;
    let (head, tail) = digits.split_at(precision);
    let mut kept: Vec<u8> = head.bytes().collect();
    let first = tail.as_bytes()[0];
    let rest_zero = tail[1..].bytes().all(|b| b == b'0');
    let round_up = first > b'5'
        || (first == b'5' && (!rest_zero || (kept.last().copied().unwrap_or(b'0') - b'0') % 2 == 1));
    let mut exponent = exponent + cut as i64;
    if round_up {
        let mut index = kept.len();
        loop {
            if index == 0 {
                kept.insert(0, b'1');
                kept.pop();
                exponent += 1;
                break;
            }
            index -= 1;
            if kept[index] == b'9' {
                kept[index] = b'0';
            } else {
                kept[index] += 1;
                break;
            }
        }
    }
    (String::from_utf8(kept).expect("digits"), exponent)
}

/// Отбросить `drop` младших цифр целого `digits` с округлением half even.
fn round_drop(digits: &str, drop: usize) -> String {
    let length = digits.len();
    if drop > length {
        return "0".into();
    }
    let (head, tail) = digits.split_at(length - drop);
    let first = tail.as_bytes().first().copied().unwrap_or(b'0');
    let rest_zero = tail.len() <= 1 || tail[1..].bytes().all(|b| b == b'0');
    let odd = head.as_bytes().last().is_some_and(|b| (b - b'0') % 2 == 1);
    let up = !tail.is_empty() && (first > b'5' || (first == b'5' && (!rest_zero || odd)));
    let mut kept: Vec<u8> = if head.is_empty() { b"0".to_vec() } else { head.bytes().collect() };
    if up {
        let mut index = kept.len();
        loop {
            if index == 0 {
                kept.insert(0, b'1');
                break;
            }
            index -= 1;
            if kept[index] == b'9' {
                kept[index] = b'0';
            } else {
                kept[index] += 1;
                break;
            }
        }
    }
    String::from_utf8(kept).expect("digits")
}

/// `BigDecimal(|value|).setScale(precision, HALF_EVEN).toPlainString()`.
pub fn fixed_text(value: f64, precision: usize) -> String {
    let (digits, exponent) = exact_decimal(value.abs());
    let shift = exponent + precision as i64;
    let integer = if shift >= 0 {
        format!("{digits}{}", "0".repeat(shift as usize))
    } else {
        round_drop(&digits, (-shift) as usize)
    };
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    if precision == 0 {
        return integer.to_string();
    }
    let padded = format!(
        "{}{integer}",
        "0".repeat((precision + 1).saturating_sub(integer.len()))
    );
    let point = padded.len() - precision;
    format!("{}.{}", &padded[..point], &padded[point..])
}

/// `BigDecimal(|value|).round(precision + 1 цифр)` в виде `d.ddd` и показатель.
pub fn scientific_parts(value: f64, precision: usize) -> (String, i64) {
    if value == 0.0 {
        return ("0".repeat(precision + 1), 0);
    }
    let (digits, exponent) = exact_decimal(value.abs());
    let (rounded, power) = round_significant(&digits, exponent, precision + 1);
    let leading = power + rounded.len() as i64 - 1;
    let mut text = rounded;
    while text.len() < precision + 1 {
        text.push('0');
    }
    (text[..precision + 1].to_string(), leading)
}

/// `sprintf("%.Ng", value)` языка C (строка числа Perl при N=15).
pub fn format_g(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Inf" } else { "-Inf" }.into();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let (digits, power) = exact_decimal(value);
    let (rounded, power) = round_significant(&digits, power, precision);
    let digits = rounded.trim_end_matches('0');
    let power = power + (rounded.len() - digits.len()) as i64;
    // Показатель старшей цифры.
    let exponent = power + digits.len() as i64 - 1;
    if exponent < -4 || exponent >= precision as i64 {
        let (first, rest) = digits.split_at(1);
        let mantissa = if rest.is_empty() {
            first.to_string()
        } else {
            format!("{first}.{rest}")
        };
        return format!(
            "{sign}{mantissa}e{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        );
    }
    if exponent < 0 {
        return format!("{sign}0.{}{digits}", "0".repeat((-exponent - 1) as usize));
    }
    let point = exponent as usize + 1;
    if digits.len() <= point {
        format!("{sign}{digits}{}", "0".repeat(point - digits.len()))
    } else {
        format!("{sign}{}.{}", &digits[..point], &digits[point..])
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixed_and_scientific_round_half_even_on_the_exact_value() {
        use super::{fixed_text, scientific_parts};
        assert_eq!(fixed_text(2.5, 0), "2");
        assert_eq!(fixed_text(3.5, 0), "4");
        assert_eq!(fixed_text(0.125, 2), "0.12");
        assert_eq!(fixed_text(0.1, 3), "0.100");
        assert_eq!(fixed_text(0.0004, 2), "0.00");
        assert_eq!(fixed_text(0.6, 0), "1");
        assert_eq!(fixed_text(9.999, 2), "10.00");
        assert_eq!(fixed_text(123.456, 1), "123.5");
        assert_eq!(scientific_parts(123.456, 2), ("123".to_string(), 2));
        assert_eq!(scientific_parts(9.99, 1), ("10".to_string(), 1));
    }

    #[test]
    fn printf_g_rounds_the_exact_binary_value() {
        use super::format_g;
        assert_eq!(format_g(0.1, 15), "0.1");
        assert_eq!(format_g(1.0 / 3.0, 15), "0.333333333333333");
        assert_eq!(format_g(1e15, 15), "1e+15");
        assert_eq!(format_g(123456789012345.0, 15), "123456789012345");
        assert_eq!(format_g(0.0001, 15), "0.0001");
        assert_eq!(format_g(0.00001, 15), "1e-05");
        assert_eq!(format_g(2.5, 1), "2");
        assert_eq!(format_g(3.5, 1), "4");
        assert_eq!(format_g(-1.5e-300, 3), "-1.5e-300");
        assert_eq!(format_g(9.9999999999999999e22, 15), "1e+23");
    }

    use super::*;

    #[test]
    fn text_follows_go_shortest_g_format() {
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "-0"),
            (1.0, "1"),
            (0.1, "0.1"),
            (1e6, "1e+06"),
            (123456.0, "123456"),
            (2.5e-7, "2.5e-07"),
            (0.0001, "0.0001"),
            (1e21, "1e+21"),
            (-1.5, "-1.5"),
        ] {
            assert_eq!(text(value), expected);
        }
    }

    #[test]
    fn conversions_check_ranges_and_traps() {
        assert_eq!(to_int(-2.9).unwrap(), IntValue::from_i64(-2));
        assert!(to_int(9.3e18).is_err());
        assert_eq!(to_byte(-0.5, None).unwrap(), 0);
        assert_eq!(
            to_byte(256.0, Some("Operand 1")).unwrap_err(),
            "Operand 1: Cannot convert float value 256 to byte: expected 0..255"
        );
        assert_eq!(modulo(-7.5, 2.0).unwrap(), -1.5);
        assert_eq!(div(1.0, 0.0).unwrap_err(), "Division by zero");
        assert_eq!(mul(1e308, 10.0).unwrap_err(), "Float value must contain a finite number");
        assert_eq!(sqrt(-1.0).unwrap_err(), "Can't take sqrt of -1");
    }
}
