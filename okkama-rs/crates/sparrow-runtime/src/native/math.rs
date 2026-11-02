//! Числовые natives — перенос `MathNatives` JVM: `runtime/format::sprintf_values`
//! (printf Perl), `runtime/statistics`, генератор PCG `runtime/random` и
//! `runtime/random_distribution`. Тексты отказов — как у Perl-provider.
//! Элементарные функции (`ln`, `exp`, `cos`, …) — libm хоста; JVM допускает
//! для `Math` погрешность в 1 ulp, так что последний бит может различаться.
use super::support::{
    array, byte_array, failed, failure, float, handle, int, items, new_handle, string, text,
};
use super::{Context, Error};
use crate::bignum::Big;
use crate::{IntValue, Value, abi::Native};
use std::cell::Cell;

fn operation(native: Native) -> &'static str {
    native.name()
}

// ---- runtime/format::sprintf_values ----

fn format_failure(native: Native, message: impl Into<String>) -> Error {
    failure("core.format", "invalid_input", native, message)
}

fn unbox(value: &Value) -> &Value {
    match value {
        Value::Any(inner) => unbox(inner),
        other => other,
    }
}

fn quoted(value: &str, quote: char) -> String {
    let mut out = String::new();
    out.push(quote);
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Исходный вид значения (`_source` Perl-provider).
fn source(native: Native, context: &Context, value: &Value) -> Result<String, Error> {
    let value = unbox(value);
    Ok(match value {
        Value::Int(value) => value.to_string(),
        Value::Float(value) => {
            let text = crate::float::format_g(*value, 15);
            if text.contains(['.', 'e', 'E']) {
                text
            } else {
                format!("{text}.0")
            }
        }
        Value::Rat(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Str(value) => quoted(value, '"'),
        Value::Char(value) => quoted(&value.to_string(), '\''),
        Value::Byte(value) => format!("byte({value})"),
        Value::Enum { declaration, case } => {
            let (name, cases) = context
                .enums
                .get(*declaration)
                .ok_or_else(|| Error::InvalidState("enum names are missing".into()))?;
            format!("{name}.{}", cases[*case])
        }
        Value::Array(array) => {
            let parts = array
                .0
                .iter()
                .map(|item| source(native, context, item))
                .collect::<Result<Vec<_>, _>>()?;
            format!("[{}]", parts.join(", "))
        }
        Value::Map(map) => {
            let parts = map
                .0
                .iter()
                .map(|(key, item)| {
                    Ok(format!(
                        "{}: {}",
                        source(native, context, &key.to_value())?,
                        source(native, context, item)?
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            format!("[{}]", parts.join(", "))
        }
        _ => return Err(failed(native)),
    })
}

fn display(native: Native, context: &Context, value: &Value) -> Result<String, Error> {
    match unbox(value) {
        Value::Str(value) => Ok(value.to_string()),
        Value::Char(value) => Ok(value.to_string()),
        other => source(native, context, other),
    }
}

/// Ширина и выравнивание (code points), флаг `0` — нули после знака.
fn pad(body: &str, flags: &str, width: usize, zero: bool) -> String {
    let length = body.chars().count();
    if length >= width {
        return body.to_string();
    }
    let fill: String = std::iter::repeat_n(if zero { '0' } else { ' ' }, width - length).collect();
    if flags.contains('-') {
        return format!("{body}{}", fill.replace('0', " "));
    }
    if zero {
        let mut signs = body.bytes().take_while(|b| b"+- ".contains(b)).count();
        let rest = &body[signs..];
        if ["0x", "0X", "0b", "0B"].iter().any(|prefix| rest.starts_with(prefix)) {
            signs += 2;
        }
        return format!("{}{fill}{}", &body[..signs], &body[signs..]);
    }
    format!("{fill}{body}")
}

/// Целое значение аргумента как IV Perl (float усекается и насыщается).
fn integer(value: &Value) -> Result<IntValue, Error> {
    Ok(match value {
        Value::Int(value) => value.clone(),
        Value::Byte(value) => IntValue::from_i64(i64::from(*value)),
        Value::Float(number) => {
            let (digits, exponent) = crate::float::exact_decimal(number.abs());
            let truncated = if exponent >= 0 {
                format!("{digits}{}", "0".repeat(exponent as usize))
            } else {
                let keep = digits.len().saturating_sub((-exponent) as usize);
                if keep == 0 {
                    "0".into()
                } else {
                    digits[..keep].to_string()
                }
            };
            let magnitude = IntValue::from_decimal_digits(&truncated).expect("digits");
            let truncated = if *number < 0.0 {
                magnitude.negated()
            } else {
                magnitude
            };
            if truncated.to_i64().is_none() && !truncated.is_negative() {
                if *number >= 1.844_674_407_370_955_2e19 {
                    IntValue::from_u128(u128::from(u64::MAX))
                } else {
                    truncated
                }
            } else if truncated.to_i64().is_none() {
                IntValue::from_i64(i64::MIN)
            } else {
                truncated
            }
        }
        _ => return Err(Error::InvalidState("expected a number".into())),
    })
}

fn radix_text(value: &IntValue, radix: u32) -> String {
    if radix == 10 {
        return value.to_string();
    }
    let mut number = value.to_big();
    if number.is_zero() {
        return "0".into();
    }
    let base = Big::from_u64(u64::from(radix));
    let mut out = Vec::new();
    while !number.is_zero() {
        let (quotient, remainder) = number.divide(&base);
        out.push(std::char::from_digit(remainder.to_i64().expect("digit") as u32, radix).expect("digit"));
        number = quotient;
    }
    out.iter().rev().collect()
}

fn format_integer(value: &Value, conversion: char, flags: &str, width: usize, precision: i64) -> Result<String, Error> {
    let number = integer(value)?;
    let signed = conversion == 'd' || conversion == 'i';
    let mut sign = "";
    let mut magnitude = number.clone();
    if signed {
        if number.is_negative() {
            sign = "-";
            magnitude = number.negated();
        } else if flags.contains('+') {
            sign = "+";
        } else if flags.contains(' ') {
            sign = " ";
        }
    } else if number.is_negative() {
        magnitude = crate::integer::add(&number, &IntValue::from_big(Big::from_u64(2).power(64)));
    }
    let radix = match conversion {
        'o' => 8,
        'x' | 'X' => 16,
        'b' => 2,
        _ => 10,
    };
    let mut digits = radix_text(&magnitude, radix);
    if conversion == 'X' {
        digits = digits.to_uppercase();
    }
    if precision >= 0 {
        if precision == 0 && magnitude.is_zero() {
            digits.clear();
        }
        while (digits.len() as i64) < precision {
            digits.insert(0, '0');
        }
    }
    let mut prefix = "";
    if flags.contains('#') && !magnitude.is_zero() {
        prefix = match conversion {
            'o' if !digits.starts_with('0') => "0",
            'x' => "0x",
            'X' => "0X",
            'b' => "0b",
            _ => "",
        };
    }
    let zero = flags.contains('0') && !flags.contains('-') && precision < 0;
    Ok(pad(&format!("{sign}{prefix}{digits}"), flags, width, zero))
}

fn number(value: &Value) -> Result<f64, Error> {
    Ok(match value {
        Value::Int(value) => value.to_f64(),
        Value::Byte(value) => f64::from(*value),
        Value::Float(value) => *value,
        _ => return Err(Error::InvalidState("expected a number".into())),
    })
}

fn exponent(value: i64) -> String {
    format!("{}{:02}", if value < 0 { '-' } else { '+' }, value.abs())
}

fn fixed(magnitude: f64, precision: usize, alternate: bool) -> String {
    let text = crate::float::fixed_text(magnitude, precision);
    if precision == 0 && alternate {
        format!("{text}.")
    } else {
        text
    }
}

fn scientific(magnitude: f64, precision: usize, alternate: bool, upper: bool) -> String {
    let (digits, exp) = crate::float::scientific_parts(magnitude, precision);
    let point = if precision > 0 || alternate { "." } else { "" };
    format!(
        "{}{point}{}{}{}",
        &digits[..1],
        &digits[1..],
        if upper { 'E' } else { 'e' },
        exponent(exp)
    )
}

fn strip_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_float(value: &Value, conversion: char, flags: &str, width: usize, precision: i64) -> Result<String, Error> {
    let number = number(value)?;
    let upper = conversion.is_ascii_uppercase();
    let mut sign = if number.is_sign_negative() && !number.is_nan() {
        "-"
    } else if flags.contains('+') {
        "+"
    } else if flags.contains(' ') {
        " "
    } else {
        ""
    };
    if !number.is_finite() {
        if number.is_nan() {
            sign = if flags.contains('+') { "+" } else { "" };
        }
        let body = if number.is_nan() { "NaN" } else { "Inf" };
        return Ok(pad(&format!("{sign}{body}"), flags, width, false));
    }
    let digits = if precision < 0 { 6 } else { precision as usize };
    let alternate = flags.contains('#');
    let magnitude = number.abs();
    let body = match conversion.to_ascii_lowercase() {
        'f' => fixed(magnitude, digits, alternate),
        'e' => scientific(magnitude, digits, alternate, upper),
        _ => {
            let significant = if digits == 0 { 1 } else { digits };
            let exp = if magnitude == 0.0 {
                0
            } else {
                crate::float::scientific_parts(magnitude, significant - 1).1
            };
            if exp < -4 || exp >= significant as i64 {
                let body = scientific(magnitude, significant - 1, alternate, upper);
                if alternate {
                    body
                } else {
                    let at = body.find(if upper { 'E' } else { 'e' }).expect("exponent");
                    format!("{}{}", strip_zeros(&body[..at]), &body[at..])
                }
            } else {
                let body = fixed(magnitude, (significant as i64 - 1 - exp).max(0) as usize, alternate);
                if alternate { body } else { strip_zeros(&body) }
            }
        }
    };
    let zero = flags.contains('0') && !flags.contains('-');
    Ok(pad(&format!("{sign}{body}"), flags, width, zero))
}

fn format_one(
    native: Native,
    context: &Context,
    value: &Value,
    conversion: char,
    flags: &str,
    width: usize,
    precision: i64,
) -> Result<String, Error> {
    let numeric = matches!(value, Value::Int(_) | Value::Float(_) | Value::Byte(_));
    match conversion {
        's' => {
            let mut body = display(native, context, value)?;
            if precision >= 0 && body.chars().count() as i64 > precision {
                body = body.chars().take(precision as usize).collect();
            }
            Ok(pad(&body, flags, width, false))
        }
        'r' => match value {
            Value::Rat(rat) => Ok(pad(&rat.to_string(), flags, width, false)),
            _ => Err(format_failure(native, "Percent r requires rat")),
        },
        'c' => {
            let point = match value {
                Value::Char(c) => i64::from(u32::from(*c)),
                // `BigInteger.intValue()`: младшие 32 бита.
                Value::Int(n) => {
                    let low = crate::integer::divide(n, &IntValue::from_i64(1 << 32))
                        .expect("non-zero")
                        .1
                        .to_i64()
                        .expect("bounded");
                    i64::from((low as u32) as i32)
                }
                Value::Byte(b) => i64::from(*b),
                _ => {
                    return Err(format_failure(
                        native,
                        "Percent c requires char, int, or byte",
                    ));
                }
            };
            let c = u32::try_from(point)
                .ok()
                .and_then(char::from_u32)
                .unwrap_or('\u{FFFD}');
            Ok(pad(&c.to_string(), flags, width, false))
        }
        'd' | 'i' | 'u' | 'o' | 'x' | 'X' | 'b' => {
            if !numeric {
                return Err(format_failure(
                    native,
                    format!("Percent {conversion} requires int, float, or byte"),
                ));
            }
            format_integer(value, conversion, flags, width, precision)
        }
        _ => {
            if !numeric {
                return Err(format_failure(
                    native,
                    format!("Percent {conversion} requires int, float, or byte"),
                ));
            }
            format_float(value, conversion, flags, width, precision)
        }
    }
}

fn sprintf(native: Native, context: &Context, pattern: &str, values: &[Value]) -> Result<String, Error> {
    let text: Vec<char> = pattern.chars().collect();
    let mut result = String::new();
    let (mut offset, mut argument) = (0, 0);
    while offset < text.len() {
        let symbol = text[offset];
        if symbol != '%' {
            result.push(symbol);
            offset += 1;
            continue;
        }
        if text.get(offset + 1) == Some(&'%') {
            result.push('%');
            offset += 2;
            continue;
        }
        offset += 1;
        let mut flags = String::new();
        while offset < text.len() && "-+ 0#".contains(text[offset]) {
            flags.push(text[offset]);
            offset += 1;
        }
        let mut width = String::new();
        while offset < text.len() && text[offset].is_ascii_digit() {
            width.push(text[offset]);
            offset += 1;
        }
        let mut precision: i64 = -1;
        if text.get(offset) == Some(&'.') {
            offset += 1;
            let mut digits = String::new();
            while offset < text.len() && text[offset].is_ascii_digit() {
                digits.push(text[offset]);
                offset += 1;
            }
            if digits.is_empty() {
                return Err(format_failure(native, "Percent precision requires digits"));
            }
            precision = digits.parse::<i32>().map_or(i64::from(i32::MAX), i64::from);
        }
        let Some(conversion) = text.get(offset).copied() else {
            return Err(format_failure(native, "Unterminated percent conversion"));
        };
        offset += 1;
        if conversion == '*' || conversion == '$' {
            return Err(format_failure(
                native,
                "Dynamic or positional percent fields are unsupported",
            ));
        }
        if !"scdiuoxXbfFeEgGr".contains(conversion) {
            return Err(format_failure(
                native,
                format!("Unsupported percent conversion {conversion}"),
            ));
        }
        if conversion == 'r' {
            if !flags.chars().all(|flag| flag == '-') {
                return Err(format_failure(native, "Percent r supports only the minus flag"));
            }
            if precision >= 0 {
                return Err(format_failure(native, "Percent r does not support precision"));
            }
        } else if (conversion == 's' || conversion == 'c') && !flags.chars().all(|flag| flag == '-') {
            return Err(format_failure(
                native,
                format!("Percent {conversion} supports only the minus flag"),
            ));
        }
        let Some(value) = values.get(argument) else {
            return Err(format_failure(native, "Missing percent format value"));
        };
        argument += 1;
        let width = width.parse::<i32>().map_or(0, |w| w.max(0) as usize);
        result.push_str(&format_one(
            native,
            context,
            unbox(value),
            conversion,
            &flags,
            width,
            precision,
        )?);
    }
    Ok(result)
}

// ---- runtime/statistics ----

fn numbers(value: &Value) -> Result<Vec<f64>, Error> {
    items(value)?.iter().map(|item| Ok(item.as_float()?)).collect()
}

fn sum(values: &[f64]) -> f64 {
    let (mut sum, mut correction) = (0.0f64, 0.0f64);
    for value in values {
        let revised = sum + value;
        correction += if sum.abs() >= value.abs() {
            (sum - revised) + value
        } else {
            (value - revised) + sum
        };
        sum = revised;
    }
    sum + correction
}

fn mean(values: &[f64]) -> f64 {
    let count = values.len() as f64;
    sum(&values.iter().map(|value| value / count).collect::<Vec<_>>())
}

fn sorted(values: &[f64]) -> Vec<f64> {
    let mut copy = values.to_vec();
    copy.sort_by(f64::total_cmp);
    copy
}

fn interpolate(left: f64, right: f64, part: i64, whole: i64) -> f64 {
    let weight = part as f64 / whole as f64;
    left * (1.0 - weight) + right * weight
}

/// Моменты пары: число, средние, суммы квадратов и произведений.
fn paired(left: &[f64], right: &[f64]) -> [f64; 6] {
    let mut count = 0.0;
    let (mut left_mean, mut right_mean) = (0.0, 0.0);
    let (mut left_squares, mut right_squares, mut products) = (0.0, 0.0, 0.0);
    for (l, r) in left.iter().zip(right) {
        count += 1.0;
        let left_delta = l - left_mean;
        let right_delta = r - right_mean;
        left_mean += left_delta / count;
        right_mean += right_delta / count;
        left_squares += left_delta * (l - left_mean);
        right_squares += right_delta * (r - right_mean);
        products += left_delta * (r - right_mean);
    }
    [count, left_mean, right_mean, left_squares, right_squares, products]
}

// ---- runtime/random: PCG XSH-RR ----

#[derive(Debug)]
struct Pcg {
    state: Cell<u64>,
}

const MULTIPLIER: u64 = 6_364_136_223_846_793_005;
const INCREMENT: u64 = 1_442_695_040_888_963_407;

impl Pcg {
    fn advance(&self) -> u64 {
        let old = self.state.get();
        self.state.set(old.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT));
        let shifted = (((old >> 18) ^ old) >> 27) as u32;
        let rotation = (old >> 59) as u32;
        u64::from(shifted.rotate_right(rotation))
    }
}

fn next_after(from: f64, toward: f64) -> f64 {
    if from == toward {
        return toward;
    }
    if from == 0.0 {
        return if toward > 0.0 { f64::from_bits(1) } else { -f64::from_bits(1) };
    }
    let bits = from.to_bits();
    let up = (toward > from) == (from > 0.0);
    f64::from_bits(if up { bits + 1 } else { bits - 1 })
}

pub fn execute(native: Native, args: &[Value], context: &Context) -> Result<Value, Error> {
    let name = operation(native);
    match name {
        "sprintf_values" => Ok(text(sprintf(
            native,
            context,
            string(&args[0])?,
            items(&args[1])?,
        )?)),
        "statistics_mean" => Ok(Value::Float(mean(&numbers(&args[0])?))),
        "statistics_geometric_mean" => {
            let values = numbers(&args[0])?;
            if values.contains(&0.0) {
                return Ok(Value::Float(0.0));
            }
            let logs: Vec<f64> = values.iter().map(|value| value.ln()).collect();
            Ok(Value::Float(mean(&logs).exp()))
        }
        "statistics_harmonic_mean" => {
            let values = numbers(&args[0])?;
            if values.contains(&0.0) {
                return Ok(Value::Float(0.0));
            }
            let smallest = sorted(&values)[0];
            let ratios: Vec<f64> = values.iter().map(|value| smallest / value).collect();
            Ok(Value::Float(smallest * values.len() as f64 / sum(&ratios)))
        }
        "statistics_median" => {
            let ordered = sorted(&numbers(&args[0])?);
            let middle = ordered.len() / 2;
            if ordered.len() % 2 == 1 {
                return Ok(Value::Float(ordered[middle]));
            }
            Ok(Value::Float(match string(&args[1])? {
                "low" => ordered[middle - 1],
                "high" => ordered[middle],
                "middle" => ordered[middle - 1] / 2.0 + ordered[middle] / 2.0,
                _ => return Err(failed(native)),
            }))
        }
        "statistics_quantiles" => {
            let ordered = sorted(&numbers(&args[0])?);
            let count = int(&args[1])?.to_i64().unwrap_or(0);
            let method = string(&args[2])?;
            let length = ordered.len() as i64;
            let mut result = Vec::new();
            match method {
                "inclusive" => {
                    let span = length - 1;
                    for index in 1..count {
                        let scaled = index * span;
                        let position = (scaled / count) as usize;
                        result.push(Value::Float(interpolate(
                            ordered[position],
                            ordered[position + 1],
                            scaled % count,
                            count,
                        )));
                    }
                }
                "exclusive" => {
                    let span = length + 1;
                    for index in 1..count {
                        let scaled = index * span;
                        let position = (scaled / count).clamp(1, length - 1) as usize;
                        result.push(Value::Float(interpolate(
                            ordered[position - 1],
                            ordered[position],
                            scaled % count,
                            count,
                        )));
                    }
                }
                _ => return Err(failed(native)),
            }
            Ok(array(result))
        }
        "statistics_variance" => {
            let values = numbers(&args[0])?;
            let sample = args[1].as_bool()?;
            let (mut count, mut mean, mut squares) = (0.0f64, 0.0f64, 0.0f64);
            for value in &values {
                count += 1.0;
                let delta = value - mean;
                mean += delta / count;
                squares += delta * (value - mean);
            }
            Ok(Value::Float(squares / if sample { count - 1.0 } else { count }))
        }
        "statistics_paired_valid" | "statistics_paired" => {
            let moments = paired(&numbers(&args[0])?, &numbers(&args[1])?);
            let result = string(&args[2])?;
            if name == "statistics_paired_valid" {
                return Ok(Value::Bool(match result {
                    "covariance" => true,
                    "correlation" => moments[3] > 0.0 && moments[4] > 0.0,
                    "slope" | "intercept" => moments[3] > 0.0,
                    _ => return Err(failed(native)),
                }));
            }
            Ok(Value::Float(match result {
                "covariance" => moments[5] / (moments[0] - 1.0),
                "correlation" => moments[5] / moments[3].sqrt() / moments[4].sqrt(),
                "slope" => moments[5] / moments[3],
                "intercept" => moments[2] - moments[5] / moments[3] * moments[1],
                _ => return Err(failed(native)),
            }))
        }
        "pcg_state" => {
            let pcg = Pcg { state: Cell::new(0) };
            pcg.advance();
            let seed = crate::integer::divide(
                int(&args[0])?,
                &IntValue::from_big(Big::from_u64(2).power(64)),
            )
            .expect("non-zero")
            .1;
            let seed = if seed.is_negative() {
                crate::integer::add(&seed, &IntValue::from_big(Big::from_u64(2).power(64)))
            } else {
                seed
            };
            let low = u64::try_from(
                u128::from_str_radix(&seed.to_string(), 10).expect("bounded seed"),
            )
            .expect("64-bit seed");
            pcg.state.set(pcg.state.get().wrapping_add(low));
            pcg.advance();
            Ok(new_handle(pcg))
        }
        "pcg_integer" => {
            let pcg = handle::<Pcg>(&args[0])?;
            let (start, stop) = (int(&args[1])?, int(&args[2])?);
            if crate::integer::compare(start, stop).is_ge() {
                return Err(failure(
                    "core.random",
                    "invalid_input",
                    native,
                    "Random range must be non-empty",
                ));
            }
            let span = crate::integer::subtract(stop, start);
            let two_64 = IntValue::from_big(Big::from_u64(2).power(64));
            let threshold = crate::integer::divide(&two_64, &span).expect("non-zero").1;
            loop {
                let high = pcg.advance();
                let low = pcg.advance();
                let sample = IntValue::from_u128((u128::from(high) << 32) + u128::from(low));
                if crate::integer::compare(&sample, &threshold).is_ge() {
                    let offset = crate::integer::divide(&sample, &span).expect("non-zero").1;
                    return Ok(Value::Int(crate::integer::add(start, &offset)));
                }
            }
        }
        "pcg_float" => {
            let pcg = handle::<Pcg>(&args[0])?;
            let high = pcg.advance() >> 5;
            let low = pcg.advance() >> 6;
            Ok(Value::Float(
                (high as f64 * 67_108_864.0 + low as f64) / 9_007_199_254_740_992.0,
            ))
        }
        "pcg_bytes" => {
            let pcg = handle::<Pcg>(&args[0])?;
            let size = int(&args[1])?;
            if size.is_negative() {
                return Err(failure(
                    "core.random",
                    "invalid_input",
                    native,
                    "Random byte size must be non-negative",
                ));
            }
            let count = size.to_i64().map_or(i32::MAX as usize, |n| n.min(i64::from(i32::MAX)) as usize);
            let mut out = Vec::with_capacity(count);
            while out.len() < count {
                let word = pcg.advance();
                let mut shift = 0;
                while shift < 32 && out.len() < count {
                    out.push((word >> shift) as u8);
                    shift += 8;
                }
            }
            Ok(byte_array(out))
        }
        "distribution_uniform" => {
            let (start, stop, fraction) = (float(&args[0])?, float(&args[1])?, float(&args[2])?);
            let mut result = start * (1.0 - fraction) + stop * fraction;
            if result < start {
                result = start;
            }
            if result >= stop {
                result = next_after(stop, start);
            }
            Ok(Value::Float(result))
        }
        "distribution_log" => {
            let value = float(&args[0])?;
            if value <= 0.0 {
                return Err(failed(native));
            }
            Ok(Value::Float(value.ln()))
        }
        "distribution_exp" => Ok(Value::Float(float(&args[0])?.exp())),
        "distribution_sqrt" => {
            let value = float(&args[0])?;
            if value < 0.0 {
                return Err(failed(native));
            }
            Ok(Value::Float(value.sqrt()))
        }
        "distribution_cos" => Ok(Value::Float(float(&args[0])?.cos())),
        "distribution_acos" => {
            let value = float(&args[0])?;
            if !(-1.0..=1.0).contains(&value) {
                return Err(failed(native));
            }
            Ok(Value::Float(value.acos()))
        }
        "distribution_power" => {
            let base = float(&args[0])?;
            if base < 0.0 {
                return Err(failed(native));
            }
            Ok(Value::Float(base.powf(float(&args[1])?)))
        }
        "distribution_angle" => {
            const TAU: f64 = 6.283_185_307_179_586;
            let mut angle = float(&args[0])? % TAU;
            if angle < 0.0 {
                angle += TAU;
            }
            Ok(Value::Float(angle))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
