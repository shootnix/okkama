//! Целые произвольной точности поверх `num-bigint` (двоичные разряды,
//! Karatsuba/Toom-3 для умножения). Общая основа `int` (большие значения),
//! `rat` и natives `runtime/bigint`/`runtime/decimal`. Семантика — как у
//! `java.math.BigInteger`: деление усекается к нулю, остаток со знаком
//! делимого.

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{Signed, ToPrimitive, Zero};
use std::cmp::Ordering;

/// Целое со знаком.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Big(BigInt);

impl Big {
    pub fn zero() -> Self {
        Self(BigInt::zero())
    }

    pub fn from_u64(value: u64) -> Self {
        Self(BigInt::from(value))
    }

    pub fn from_i64(value: i64) -> Self {
        Self(BigInt::from(value))
    }

    /// `-?[0-9]+` (ведущие нули допустимы, как у `BigInteger`).
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('-').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        BigInt::parse_bytes(text.as_bytes(), 10).map(Self)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn sign(&self) -> i64 {
        match self.0.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    pub fn negated(&self) -> Self {
        Self(-&self.0)
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    pub fn add(&self, other: &Self) -> Self {
        Self(&self.0 + &other.0)
    }

    pub fn subtract(&self, other: &Self) -> Self {
        Self(&self.0 - &other.0)
    }

    pub fn multiply(&self, other: &Self) -> Self {
        Self(&self.0 * &other.0)
    }

    /// Частное к нулю и остаток со знаком делимого (`right` не ноль).
    pub fn divide(&self, other: &Self) -> (Self, Self) {
        let (quotient, remainder) = self.0.div_rem(&other.0);
        (Self(quotient), Self(remainder))
    }

    pub fn gcd(&self, other: &Self) -> Self {
        Self(self.0.gcd(&other.0))
    }

    /// Битовая длина модуля (`BigInteger.abs().bitLength()`).
    pub fn bits(&self) -> u64 {
        self.0.magnitude().bits()
    }

    pub fn power(&self, exponent: u32) -> Self {
        Self(num_traits::pow::Pow::pow(&self.0, exponent))
    }

    pub fn to_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }
}

impl std::fmt::Display for Big {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::Big;

    #[test]
    fn division_truncates_and_text_round_trips() {
        let big = Big::parse("-123456789012345678901234567890").unwrap();
        let (quotient, remainder) = big.divide(&Big::from_i64(7));
        assert_eq!(quotient.to_string(), "-17636684144620811271604938270");
        assert_eq!(remainder.to_string(), "0");
        let (quotient, remainder) = Big::from_i64(-7).divide(&Big::from_i64(2));
        assert_eq!((quotient.to_string(), remainder.to_string()), ("-3".into(), "-1".into()));
        assert_eq!(Big::parse("-000").unwrap().to_string(), "0");
        assert!(Big::parse("+1").is_none() && Big::parse("").is_none() && Big::parse("1e3").is_none());
        assert_eq!(Big::from_u64(2).power(64).bits(), 65);
        assert_eq!(Big::parse("-12").unwrap().gcd(&Big::from_i64(18)).to_string(), "6");
    }
}
