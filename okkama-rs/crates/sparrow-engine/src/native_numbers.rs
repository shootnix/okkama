//! Exact BigInt native operations use the same Rust arithmetic as VM integers.
use crate::{Error, native::Outcome, value::*};
use num_bigint::{BigInt, Sign};
use num_integer::Integer as _;
use num_traits::{Signed, ToPrimitive, Zero};
use okkama_runtime_abi::{Native, NativeFailure};
pub(crate) fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        DecimalValid
            | DecimalCanonical
            | DecimalCoefficient
            | DecimalExponent
            | DecimalOrder
            | BigintValid
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
    )
}
fn parse(source: &str) -> Option<BigInt> {
    let digits = source.strip_prefix('-').unwrap_or(source);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    BigInt::parse_bytes(source.as_bytes(), 10)
}
impl Heap {
    fn bigint_failure(
        &mut self,
        native: Native,
        code: &str,
        message: &str,
    ) -> Result<Outcome, Error> {
        let payload = self.native_failure_new(NativeFailure {
            domain: "core.bigint".into(),
            code: code.into(),
            operation: native.symbol().into(),
            message: message.into(),
        })?;
        self.failure_new(crate::failure::FailureKind::Native(payload))
            .map(Outcome::Failure)
    }
    pub(crate) fn native_numbers(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        use Native::*;
        if matches!(
            native,
            DecimalValid | DecimalCanonical | DecimalCoefficient | DecimalExponent | DecimalOrder
        ) {
            let parts = components(self.text(args[0])?);
            if native == DecimalValid {
                return Ok(Outcome::Value(Slot::boolean(parts.is_some())));
            }
            let invalid = || Error(format!("Native invocation \"{}\" failed", native.symbol()));
            let left = parts.ok_or_else(invalid)?;
            let value = match native {
                DecimalCanonical => self.allocate_text(canonical(&left))?,
                DecimalCoefficient => self.allocate_text(left.coefficient)?,
                DecimalExponent => Slot::small(left.exponent),
                DecimalOrder => {
                    let right = components(self.text(args[1])?).ok_or_else(invalid)?;
                    let mut order = left.sign.cmp(&right.sign);
                    if order.is_eq() && left.sign != 0 {
                        order = (left.digits.len() as i128 + left.exponent as i128)
                            .cmp(&(right.digits.len() as i128 + right.exponent as i128));
                        if order.is_eq() {
                            let width = left.digits.len().max(right.digits.len());
                            order = format!("{:0<width$}", left.digits)
                                .cmp(&format!("{:0<width$}", right.digits));
                        }
                        if left.sign < 0 {
                            order = order.reverse();
                        }
                    }
                    Slot::small(match order {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    })
                }
                _ => unreachable!(),
            };
            return Ok(Outcome::Value(value));
        }
        let number = |slot| {
            parse(self.text(slot)?)
                .ok_or_else(|| Error(format!("Native invocation \"{}\" failed", native.symbol())))
        };
        if native == BigintValid {
            return Ok(Outcome::Value(Slot::boolean(
                parse(self.text(args[0])?).is_some(),
            )));
        }
        let left = number(args[0])?;
        match native {
            BigintSign => {
                return Ok(Outcome::Value(Slot::small(match left.sign() {
                    Sign::Minus => -1,
                    Sign::NoSign => 0,
                    Sign::Plus => 1,
                })));
            }
            BigintBits => return self.allocate(BigInt::from(left.bits())).map(Outcome::Value),
            BigintCanonical => return self.allocate_text(left.to_string()).map(Outcome::Value),
            BigintPower => {
                let big = |slot| {
                    self.integer(slot).map(|n| match n {
                        Integer::Small(n) => BigInt::from(n),
                        Integer::Big(n) => (*n).clone(),
                    })
                };
                let exponent = big(args[1])?;
                let maximum = big(args[2])?;
                if exponent.is_negative() {
                    return self.bigint_failure(
                        native,
                        "invalid_input",
                        "BigInt exponent must be non-negative",
                    );
                }
                if maximum <= BigInt::zero() {
                    return self.bigint_failure(
                        native,
                        "invalid_input",
                        "BigInt power bit limit must be positive",
                    );
                }
                let result = if exponent.is_zero() {
                    BigInt::from(1)
                } else if left.is_zero() {
                    BigInt::zero()
                } else if left.abs() == BigInt::from(1) {
                    if left.is_negative() && exponent.is_odd() {
                        BigInt::from(-1)
                    } else {
                        BigInt::from(1)
                    }
                } else {
                    let lower = &exponent * BigInt::from(left.bits() - 1) + 1;
                    if lower > maximum || exponent.bits() > 31 {
                        return self.bigint_failure(
                            native,
                            "resource_exhausted",
                            "BigInt power exceeds bit limit",
                        );
                    }
                    let result = left.pow(exponent.to_u32().expect("31-bit exponent"));
                    if BigInt::from(result.bits()) > maximum {
                        return self.bigint_failure(
                            native,
                            "resource_exhausted",
                            "BigInt power exceeds bit limit",
                        );
                    }
                    result
                };
                return self.allocate_text(result.to_string()).map(Outcome::Value);
            }
            _ => {}
        }
        let right = number(args[1])?;
        let result = match native {
            BigintOrder => {
                return Ok(Outcome::Value(Slot::small(match left.cmp(&right) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })));
            }
            BigintSum => left + right,
            BigintDifference => left - right,
            BigintProduct => left * right,
            BigintGcd => left.gcd(&right),
            BigintLcm => left.lcm(&right),
            BigintQuotient | BigintRemainder => {
                if right.is_zero() {
                    return self.bigint_failure(native, "invalid_input", "BigInt division by zero");
                }
                if native == BigintQuotient {
                    left / right
                } else {
                    left % right
                }
            }
            _ => return Err(Error("invalid BigInt provider".into())),
        };
        self.allocate_text(result.to_string()).map(Outcome::Value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_division_magnitude_bits_and_power_limits_match_the_language() {
        let mut heap = Heap::new(&[], &[]);
        let left = heap.allocate_text("-17".into()).unwrap();
        let right = heap.allocate_text("5".into()).unwrap();
        for (native, expected) in [
            (Native::BigintQuotient, "-3"),
            (Native::BigintRemainder, "-2"),
            (Native::BigintGcd, "1"),
            (Native::BigintLcm, "85"),
        ] {
            let Outcome::Value(value) = heap.native_numbers(native, &[left, right]).unwrap() else {
                panic!("value")
            };
            assert_eq!(heap.text(value).unwrap(), expected);
        }
        let Outcome::Value(bits) = heap.native_numbers(Native::BigintBits, &[left]).unwrap() else {
            panic!("bits")
        };
        assert_eq!(bits.bits, 5);
        let two = heap.allocate_text("2".into()).unwrap();
        let Outcome::Failure(failure) = heap
            .native_numbers(
                Native::BigintPower,
                &[two, Slot::small(10), Slot::small(10)],
            )
            .unwrap()
        else {
            panic!("limit")
        };
        let crate::failure::FailureKind::Native(payload) = heap.failure(failure).unwrap().kind
        else {
            panic!("native")
        };
        assert_eq!(
            heap.native_failure(payload).unwrap().code,
            "resource_exhausted"
        );
        let Outcome::Value(value) = heap
            .native_numbers(
                Native::BigintPower,
                &[two, Slot::small(10), Slot::small(11)],
            )
            .unwrap()
        else {
            panic!("power")
        };
        assert_eq!(heap.text(value).unwrap(), "1024");
    }
}

/// Canonical decimal components shared with JSON; exponents remain exact during
/// parsing, then checked against the language's i64 exponent contract.
pub(crate) struct Decimal {
    pub(crate) coefficient: String,
    pub(crate) exponent: i64,
    pub(crate) digits: String,
    pub(crate) sign: i64,
}
pub(crate) fn components(source: &str) -> Option<Decimal> {
    let (negative, rest) = source
        .strip_prefix('-')
        .map_or((false, source), |s| (true, s));
    let integer_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if integer_end == 0 {
        return None;
    }
    let integer = &rest[..integer_end];
    let mut tail = &rest[integer_end..];
    let mut fraction = "";
    if let Some(after) = tail.strip_prefix('.') {
        let end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        if end == 0 {
            return None;
        }
        fraction = &after[..end];
        tail = &after[end..];
    }
    let mut written = BigInt::zero();
    if let Some(after) = tail.strip_prefix(['e', 'E']) {
        let (negative, digits) = match after.as_bytes().first() {
            Some(b'+') => (false, &after[1..]),
            Some(b'-') => (true, &after[1..]),
            _ => (false, after),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        written = BigInt::parse_bytes(digits.as_bytes(), 10)?;
        if negative {
            written = -written;
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
            digits: "0".into(),
            exponent: 0,
            sign: 0,
        });
    }
    let trimmed = digits.trim_end_matches('0');
    let exponent = (written - BigInt::from(fraction.len())
        + BigInt::from(digits.len() - trimmed.len()))
    .to_i64()?;
    Some(Decimal {
        coefficient: format!("{}{trimmed}", if negative { "-" } else { "" }),
        exponent,
        digits: trimmed.into(),
        sign: if negative { -1 } else { 1 },
    })
}
pub(crate) fn canonical(parts: &Decimal) -> String {
    if parts.sign == 0 {
        return "0".into();
    }
    let mut out = String::new();
    if parts.sign < 0 {
        out.push('-');
    }
    out.push_str(&parts.digits[..1]);
    if parts.digits.len() > 1 {
        out.push('.');
        out.push_str(&parts.digits[1..]);
    }
    let exponent = parts.exponent as i128 + parts.digits.len() as i128 - 1;
    if exponent != 0 {
        out.push_str(&format!("e{exponent}"));
    }
    out
}
#[cfg(test)]
mod decimal_tests {
    use super::*;
    #[test]
    fn decimal_components_preserve_exact_exponents_zero_and_spelling_rules() {
        for (source, expected) in [
            ("000.0012300e+5", "1.23e2"),
            ("-12.50", "-1.25e1"),
            ("0e99999999999999999999999999999", "0"),
            ("1e9223372036854775807", "1e9223372036854775807"),
        ] {
            assert_eq!(canonical(&components(source).unwrap()), expected);
        }
        for source in [
            "+1",
            ".1",
            "1.",
            "1e",
            "1e+",
            "1_0",
            "١",
            "1e9223372036854775808",
        ] {
            assert!(components(source).is_none(), "{source}");
        }
    }
}
