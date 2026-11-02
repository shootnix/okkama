//! Точное рациональное `rat` (OIR-1 §16.2): несократимая дробь с
//! положительным знаменателем. Текст — `n/d`, у целого — `n`. Ошибки и
//! преобразования — как у JVM-рантайма (`okkama.rt.Rat`).

use crate::IntValue;
use crate::integer::{add, compare, divide, gcd, multiply, subtract};
use std::cmp::Ordering;
use std::fmt;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rat {
    num: IntValue,
    den: IntValue,
}

fn one() -> IntValue {
    IntValue::from_i64(1)
}

fn pow10(exponent: u32) -> IntValue {
    IntValue::from_big(crate::bignum::Big::from_u64(10).power(exponent))
}

impl Rat {
    fn normalized(mut num: IntValue, mut den: IntValue) -> Self {
        if den.is_negative() {
            num = num.negated();
            den = den.negated();
        }
        let common = gcd(&num, &den);
        if !common.is_zero() && common != one() {
            num = divide(&num, &common).expect("non-zero gcd").0;
            den = divide(&den, &common).expect("non-zero gcd").0;
        }
        if num.is_zero() {
            den = one();
        }
        Self { num, den }
    }

    /// `rat(n, d)` и `n / d` над целыми.
    pub fn new(num: &IntValue, den: &IntValue) -> Result<Self> {
        if den.is_zero() {
            return Err("Division by zero".into());
        }
        Ok(Self::normalized(num.clone(), den.clone()))
    }

    pub fn from_int(value: &IntValue) -> Self {
        Self {
            num: value.clone(),
            den: one(),
        }
    }

    /// `float.rat()`: точная дробь канонической десятичной записи float.
    pub fn from_float(value: f64) -> Self {
        let text = crate::float::text(value);
        let (mantissa, exponent) = match text.split_once('e') {
            Some((mantissa, exponent)) => (
                mantissa.to_string(),
                exponent.parse::<i32>().expect("float text exponent"),
            ),
            None => (text.clone(), 0),
        };
        let (negative, mantissa) = match mantissa.strip_prefix('-') {
            Some(rest) => (true, rest.to_string()),
            None => (false, mantissa),
        };
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((&mantissa, ""));
        let digits = IntValue::from_decimal_digits(&format!("{whole}{fraction}"))
            .expect("float text digits");
        let digits = if negative { digits.negated() } else { digits };
        let scale = fraction.len() as i32 - exponent;
        if scale <= 0 {
            Self {
                num: multiply(&digits, &pow10(scale.unsigned_abs())),
                den: one(),
            }
        } else {
            Self::normalized(digits, pow10(scale as u32))
        }
    }

    pub fn add(&self, other: &Self) -> Self {
        Self::normalized(
            add(
                &multiply(&self.num, &other.den),
                &multiply(&other.num, &self.den),
            ),
            multiply(&self.den, &other.den),
        )
    }

    pub fn sub(&self, other: &Self) -> Self {
        Self::normalized(
            subtract(
                &multiply(&self.num, &other.den),
                &multiply(&other.num, &self.den),
            ),
            multiply(&self.den, &other.den),
        )
    }

    pub fn mul(&self, other: &Self) -> Self {
        Self::normalized(
            multiply(&self.num, &other.num),
            multiply(&self.den, &other.den),
        )
    }

    pub fn div(&self, other: &Self) -> Result<Self> {
        if other.num.is_zero() {
            return Err("Division by zero".into());
        }
        Ok(Self::normalized(
            multiply(&self.num, &other.den),
            multiply(&self.den, &other.num),
        ))
    }

    /// Остаток со знаком делимого: `a - b * trunc(a / b)` (D-031).
    pub fn modulo(&self, other: &Self) -> Result<Self> {
        if other.num.is_zero() {
            return Err("Modulo by zero".into());
        }
        let quotient = divide(
            &multiply(&self.num, &other.den),
            &multiply(&self.den, &other.num),
        )
        .expect("non-zero divisor")
        .0;
        Ok(self.sub(&other.mul(&Self::from_int(&quotient))))
    }

    pub fn neg(&self) -> Self {
        Self {
            num: self.num.negated(),
            den: self.den.clone(),
        }
    }

    pub fn abs(&self) -> Self {
        if self.num.is_negative() {
            self.neg()
        } else {
            self.clone()
        }
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        compare(
            &multiply(&self.num, &other.den),
            &multiply(&other.num, &self.den),
        )
    }

    pub fn sign(&self) -> i64 {
        self.num.signum()
    }

    pub fn numerator(&self) -> &IntValue {
        &self.num
    }

    pub fn denominator(&self) -> &IntValue {
        &self.den
    }

    /// Усечение к нулю.
    pub fn to_int(&self) -> IntValue {
        divide(&self.num, &self.den).expect("positive denominator").0
    }

    /// Как `BigDecimal.divide(MathContext.DECIMAL128).doubleValue()`: частное,
    /// округлённое до 34 значащих цифр (half even), затем ближайшее binary64.
    pub fn to_float(&self) -> Result<f64> {
        let result = if self.num.is_zero() {
            0.0
        } else {
            let magnitude = if self.num.is_negative() {
                self.num.negated()
            } else {
                self.num.clone()
            };
            let digits = |value: &IntValue| value.to_string().len() as i64;
            // Ищем k: 10^33 <= |n| * 10^k / d < 10^34.
            let mut k = 33 - (digits(&magnitude) - digits(&self.den));
            let low = pow10(33);
            let (quotient, rest, divisor) = loop {
                let (scaled, divisor) = if k >= 0 {
                    (multiply(&magnitude, &pow10(k as u32)), self.den.clone())
                } else {
                    (magnitude.clone(), multiply(&self.den, &pow10((-k) as u32)))
                };
                let (quotient, rest) = divide(&scaled, &divisor).expect("positive divisor");
                if compare(&quotient, &low).is_lt() {
                    k += 1;
                } else if quotient.to_string().len() > 34 {
                    k -= 1;
                } else {
                    break (quotient, rest, divisor);
                }
            };
            let twice = multiply(&rest, &IntValue::from_i64(2));
            let odd = || !divide(&quotient, &IntValue::from_i64(2)).expect("two").1.is_zero();
            let quotient = match compare(&twice, &divisor) {
                Ordering::Greater => add(&quotient, &one()),
                Ordering::Equal if odd() => add(&quotient, &one()),
                _ => quotient,
            };
            let sign = if self.num.is_negative() { "-" } else { "" };
            format!("{sign}{quotient}e{}", -k)
                .parse::<f64>()
                .expect("decimal text is a float")
        };
        if !result.is_finite() {
            return Err("Cannot convert rat to float: expected a finite binary64 value".into());
        }
        Ok(result)
    }

    /// Усечение к нулю, затем 0..255; `context` — номер операнда `@byte()`.
    pub fn to_byte(&self, context: Option<&str>) -> Result<u8> {
        let truncated = self.to_int();
        truncated
            .to_i64()
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| {
                let message = format!("Cannot convert rat value {self} to byte: expected 0..255");
                match context {
                    Some(context) => format!("{context}: {message}"),
                    None => message,
                }
            })
    }
}

impl fmt::Display for Rat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == one() {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rat(n: i64, d: i64) -> Rat {
        Rat::new(&IntValue::from_i64(n), &IntValue::from_i64(d)).unwrap()
    }

    #[test]
    fn fractions_are_reduced_with_positive_denominators() {
        assert_eq!(rat(6, -4).to_string(), "-3/2");
        assert_eq!(rat(0, -5).to_string(), "0");
        assert_eq!(rat(1, 2).add(&rat(1, 3)).to_string(), "5/6");
        assert_eq!(rat(-7, 2).modulo(&rat(1, 1)).unwrap().to_string(), "-1/2");
        assert_eq!(rat(-7, 2).to_int(), IntValue::from_i64(-3));
        assert!(rat(1, 3).div(&rat(0, 1)).is_err());
    }

    #[test]
    fn float_conversions_use_canonical_decimal_text() {
        assert_eq!(Rat::from_float(0.1).to_string(), "1/10");
        assert_eq!(Rat::from_float(1e21).to_string(), "1000000000000000000000");
        assert_eq!(Rat::from_float(-2.5e-7).to_string(), "-1/4000000");
        assert_eq!(rat(1, 3).to_float().unwrap(), 1.0 / 3.0);
        assert_eq!(rat(-2, 3).to_float().unwrap(), -2.0 / 3.0);
        assert_eq!(rat(1, 10).to_float().unwrap(), 0.1);
        assert_eq!(
            rat(300, 1).to_byte(None).unwrap_err(),
            "Cannot convert rat value 300 to byte: expected 0..255"
        );
    }
}
