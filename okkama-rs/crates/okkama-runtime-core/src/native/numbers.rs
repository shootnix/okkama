//! `runtime/bigint` и `runtime/decimal` — перенос `BigIntNatives` и части
//! `ValueNatives` JVM-рантайма. Целые произвольной точности — свой `Big`
//! (основание 10^9), без внешних зависимостей. Неверная запись операнда —
//! `Native invocation "…" failed`; ожидаемые сбои — `NativeFailure`
//! (`core.bigint`).

use super::Error;
use crate::{IntValue, Value, abi::Native, abi::NativeFailure};
use crate::bignum::Big;
use std::cmp::Ordering;
use std::rc::Rc;


fn failed(native: Native) -> Error {
    Error::Trap(format!("Native invocation \"{}\" failed", native.symbol()))
}

fn failure(native: Native, code: &str, message: &str) -> Error {
    Error::Failure(NativeFailure {
        domain: "core.bigint".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: message.into(),
    })
}

fn number(native: Native, value: &Value) -> Result<Big, Error> {
    Big::parse(value.as_string()?).ok_or_else(|| failed(native))
}

fn text(value: impl Into<String>) -> Value {
    Value::Str(Rc::from(value.into()))
}

fn int(value: i64) -> Value {
    Value::Int(IntValue::from_i64(value))
}

fn big_int(value: &IntValue) -> Big {
    value.to_big()
}

/// Разбор `_components` decimal: значащие цифры без нулей по краям и порядок.
pub(super) struct Decimal {
    pub(super) coefficient: String,
    pub(super) exponent: i64,
    pub(super) digits: String,
    pub(super) sign: i64,
}

/// Каноническая запись decimal: `0`, иначе мантисса `d[.ddd]` и `e<порядок>`,
/// если порядок не нулевой (`1e3`, `-1.25e1`, `1.5`).
pub(super) fn canonical(parts: &Decimal) -> String {
    if parts.sign == 0 {
        return "0".into();
    }
    let mut mantissa = String::new();
    if parts.sign < 0 {
        mantissa.push('-');
    }
    mantissa.push_str(&parts.digits[..1]);
    if parts.digits.len() > 1 {
        mantissa.push('.');
        mantissa.push_str(&parts.digits[1..]);
    }
    let scientific = i128::from(parts.exponent) + parts.digits.len() as i128 - 1;
    if scientific != 0 {
        mantissa.push_str(&format!("e{scientific}"));
    }
    mantissa
}

pub(super) fn components(source: &str) -> Option<Decimal> {
    // (-?)([0-9]+)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?
    let (negative, rest) = match source.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, source),
    };
    let integer_end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    if integer_end == 0 {
        return None;
    }
    let integer = &rest[..integer_end];
    let mut tail = &rest[integer_end..];
    let mut fraction = "";
    if let Some(after) = tail.strip_prefix('.') {
        let end = after
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(after.len());
        if end == 0 {
            return None;
        }
        fraction = &after[..end];
        tail = &after[end..];
    }
    let mut written = Big::zero();
    if let Some(after) = tail.strip_prefix(['e', 'E']) {
        let (negative, digits) = match after.as_bytes().first() {
            Some(b'+') => (false, &after[1..]),
            Some(b'-') => (true, &after[1..]),
            _ => (false, after),
        };
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        written = Big::parse(digits)?;
        if negative {
            written = written.negated();
        }
        tail = "";
    }
    if !tail.is_empty() {
        return None;
    }
    let all = format!("{integer}{fraction}");
    let digits = all.trim_start_matches('0');
    if digits.is_empty() {
        return Some(Decimal {
            coefficient: "0".into(),
            exponent: 0,
            digits: "0".into(),
            sign: 0,
        });
    }
    let trimmed = digits.trim_end_matches('0');
    let exponent = written
        .subtract(&Big::from_u64(fraction.len() as u64))
        .add(&Big::from_u64((digits.len() - trimmed.len()) as u64));
    let exponent = exponent.to_i64()?;
    Some(Decimal {
        coefficient: format!("{}{trimmed}", if negative { "-" } else { "" }),
        exponent,
        digits: trimmed.to_string(),
        sign: if negative { -1 } else { 1 },
    })
}

fn decimal(native: Native, value: &Value) -> Result<Decimal, Error> {
    components(value.as_string()?).ok_or_else(|| failed(native))
}

pub fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        BigintValid
            | BigintCanonical
            | BigintSign
            | BigintBits
            | BigintDifference
            | BigintOrder
            | BigintSum
            | BigintProduct
            | BigintQuotient
            | BigintRemainder
            | BigintGcd
            | BigintLcm
            | BigintPower
            | DecimalValid
            | DecimalCanonical
            | DecimalCoefficient
            | DecimalExponent
            | DecimalOrder
    )
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    use Native::*;
    let order = |ordering: Ordering| int(ordering as i64);
    Ok(match native {
        BigintValid => Value::Bool(Big::parse(args[0].as_string()?).is_some()),
        BigintCanonical => text(number(native, &args[0])?.to_string()),
        BigintSign => int(number(native, &args[0])?.sign()),
        BigintBits => int(number(native, &args[0])?.bits() as i64),
        BigintDifference => text(
            number(native, &args[0])?
                .subtract(&number(native, &args[1])?)
                .to_string(),
        ),
        BigintOrder => order(number(native, &args[0])?.compare(&number(native, &args[1])?)),
        BigintSum => text(
            number(native, &args[0])?
                .add(&number(native, &args[1])?)
                .to_string(),
        ),
        BigintProduct => text(
            number(native, &args[0])?
                .multiply(&number(native, &args[1])?)
                .to_string(),
        ),
        BigintQuotient | BigintRemainder => {
            let (left, right) = (number(native, &args[0])?, number(native, &args[1])?);
            if right.is_zero() {
                return Err(failure(native, "invalid_input", "BigInt division by zero"));
            }
            let (quotient, remainder) = left.divide(&right);
            text(
                if native == BigintQuotient {
                    quotient
                } else {
                    remainder
                }
                .to_string(),
            )
        }
        BigintGcd => text(
            number(native, &args[0])?
                .gcd(&number(native, &args[1])?)
                .to_string(),
        ),
        BigintLcm => {
            let (left, right) = (
                number(native, &args[0])?.abs(),
                number(native, &args[1])?.abs(),
            );
            if left.is_zero() || right.is_zero() {
                text("0")
            } else {
                text(
                    left.divide(&left.gcd(&right))
                        .0
                        .multiply(&right)
                        .to_string(),
                )
            }
        }
        BigintPower => {
            let exponent = big_int(args[1].as_int()?);
            let maximum = big_int(args[2].as_int()?);
            if exponent.sign() < 0 {
                return Err(failure(
                    native,
                    "invalid_input",
                    "BigInt exponent must be non-negative",
                ));
            }
            if maximum.sign() <= 0 {
                return Err(failure(
                    native,
                    "invalid_input",
                    "BigInt power bit limit must be positive",
                ));
            }
            let base = number(native, &args[0])?;
            if exponent.is_zero() {
                return Ok(text("1"));
            }
            if base.is_zero() {
                return Ok(text("0"));
            }
            let absolute = base.abs();
            if absolute == Big::from_u64(1) {
                let odd = {
                    let (_, remainder) = exponent.divide(&Big::from_u64(2));
                    !remainder.is_zero()
                };
                return Ok(text(if base.sign() < 0 && odd { "-1" } else { "1" }));
            }
            let exceeded = || {
                failure(
                    native,
                    "resource_exhausted",
                    "BigInt power exceeds bit limit",
                )
            };
            // У k-битного |base| >= 2 в результате не меньше (k-1)*exponent+1 бит.
            let lower = exponent
                .multiply(&Big::from_u64(absolute.bits() - 1))
                .add(&Big::from_u64(1));
            if lower.compare(&maximum) == Ordering::Greater || exponent.bits() > 31 {
                return Err(exceeded());
            }
            let exponent = exponent.to_i64().expect("31-bit exponent") as u32;
            let result = base.power(exponent);
            if Big::from_u64(result.bits()).compare(&maximum) == Ordering::Greater {
                return Err(exceeded());
            }
            text(result.to_string())
        }
        DecimalValid => Value::Bool(components(args[0].as_string()?).is_some()),
        DecimalCanonical => text(canonical(&decimal(native, &args[0])?)),
        DecimalCoefficient => text(decimal(native, &args[0])?.coefficient),
        DecimalExponent => int(decimal(native, &args[0])?.exponent),
        DecimalOrder => {
            let (left, right) = (decimal(native, &args[0])?, decimal(native, &args[1])?);
            if left.sign != right.sign {
                return Ok(order(left.sign.cmp(&right.sign)));
            }
            if left.sign == 0 {
                return Ok(int(0));
            }
            let magnitude =
                |parts: &Decimal| parts.digits.len() as i128 + i128::from(parts.exponent);
            let mut comparison = magnitude(&left).cmp(&magnitude(&right));
            if comparison == Ordering::Equal {
                let width = left.digits.len().max(right.digits.len());
                comparison =
                    format!("{:0<width$}", left.digits).cmp(&format!("{:0<width$}", right.digits));
            }
            order(if left.sign < 0 {
                comparison.reverse()
            } else {
                comparison
            })
        }
        _ => return Err(Error::InvalidState("not a number native".into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(text: &str) -> Big {
        Big::parse(text).unwrap()
    }

    #[test]
    fn arithmetic_matches_small_integers() {
        for a in -60i64..=60 {
            for b in -60i64..=60 {
                let (x, y) = (Big::from_i64(a), Big::from_i64(b));
                assert_eq!(x.add(&y), Big::from_i64(a + b));
                assert_eq!(x.subtract(&y), Big::from_i64(a - b));
                assert_eq!(x.multiply(&y), Big::from_i64(a * b));
                assert_eq!(x.compare(&y), a.cmp(&b));
                if b != 0 {
                    let (q, r) = x.divide(&y);
                    assert_eq!(
                        (q, r),
                        (Big::from_i64(a / b), Big::from_i64(a % b)),
                        "{a} / {b}"
                    );
                }
            }
        }
    }

    #[test]
    fn large_values_divide_gcd_and_measure_bits() {
        let a = big("123456789012345678901234567890123456789");
        let b = big("987654321987654321");
        let (q, r) = a.divide(&b);
        assert_eq!(q.multiply(&b).add(&r), a);
        assert_eq!(q.to_string(), "124999998748437501153");
        assert_eq!(big("-007").to_string(), "-7");
        assert_eq!(big("-0").to_string(), "0");
        assert_eq!(big("340282366920938463463374607431768211456").bits(), 129);
        assert_eq!(big("255").bits(), 8);
        assert_eq!(big("0").bits(), 0);
        assert_eq!(big("1071").gcd(&big("-462")).to_string(), "21");
        assert_eq!(
            big("2").power(100).to_string(),
            "1267650600228229401496703205376"
        );
    }

    #[test]
    fn decimal_components_match_the_jvm_parser() {
        let parts = components("-001.2000").unwrap();
        assert_eq!((parts.coefficient.as_str(), parts.exponent), ("-12", -1));
        let parts = components("1000").unwrap();
        assert_eq!((parts.digits.as_str(), parts.exponent), ("1", 3));
        assert!(components("1.").is_none());
        assert!(components(".5").is_none());
        assert!(components("1e").is_none());
        assert!(components("1e+-5").is_none());
        assert_eq!(components("15e-3").unwrap().exponent, -3);
        assert_eq!(components("0.000").unwrap().sign, 0);
    }
}
