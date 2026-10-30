//! Shared runtime values; no source syntax, IR, or execution-engine dependency.

pub mod bignum;
pub mod data;
pub mod failure;
pub mod float;
pub mod gc;
pub mod rat;
pub mod heap;
pub mod host;
pub mod native;
pub mod ops;
pub mod range;
pub use okkama_runtime_abi as abi;
pub mod integer;
pub mod intrinsics;
pub mod scalar;
pub mod sched;
pub mod strings;
pub mod testing;
pub mod worker;
mod value;
pub use value::{Value, ValueError};

/// Точное целое (R01): машинное слово, пока значение помещается в signed-64,
/// иначе — `bignum::Big`. Инвариант: `Big` только вне диапазона i64, поэтому
/// равенство и хеш структурные.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct IntValue(Repr);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[repr(C, u8)]
enum Repr {
    Small(i64),
    Big(std::sync::Arc<bignum::Big>),
}

impl IntValue {
    /// Принимает только каноническую десятичную запись.
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('-').unwrap_or(text);
        let canonical = !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && (digits == "0" || !digits.starts_with('0'))
            && text != "-0";
        if !canonical {
            return None;
        }
        Some(match text.parse::<i64>() {
            Ok(value) => Self(Repr::Small(value)),
            Err(_) => Self::from_big(bignum::Big::parse(text)?),
        })
    }

    /// Нормализует неотрицательную запись из цифр, удаляя ведущие нули.
    pub fn from_decimal_digits(digits: &str) -> Option<Self> {
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        bignum::Big::parse(digits).map(Self::from_big)
    }

    pub fn from_u128(value: u128) -> Self {
        match i64::try_from(value) {
            Ok(value) => Self(Repr::Small(value)),
            Err(_) => Self::from_big(bignum::Big::parse(&value.to_string()).expect("digits")),
        }
    }

    pub fn from_i128(value: i128) -> Self {
        match i64::try_from(value) {
            Ok(value) => Self(Repr::Small(value)),
            Err(_) => Self::from_big(bignum::Big::parse(&value.to_string()).expect("digits")),
        }
    }

    #[inline(always)]
    pub fn from_i64(value: i64) -> Self {
        Self(Repr::Small(value))
    }

    pub fn from_big(value: bignum::Big) -> Self {
        match value.to_i64() {
            Some(small) => Self(Repr::Small(small)),
            None => Self(Repr::Big(std::sync::Arc::new(value))),
        }
    }

    /// Значение как `Big` без копии разрядов большого числа.
    pub fn as_big(&self) -> std::borrow::Cow<'_, bignum::Big> {
        match &self.0 {
            Repr::Small(value) => std::borrow::Cow::Owned(bignum::Big::from_i64(*value)),
            Repr::Big(value) => std::borrow::Cow::Borrowed(value),
        }
    }

    pub fn to_big(&self) -> bignum::Big {
        match &self.0 {
            Repr::Small(value) => bignum::Big::from_i64(*value),
            Repr::Big(value) => (**value).clone(),
        }
    }

    pub fn negated(&self) -> Self {
        match &self.0 {
            Repr::Small(value) => match value.checked_neg() {
                Some(value) => Self(Repr::Small(value)),
                None => Self::from_i128(-i128::from(*value)),
            },
            Repr::Big(value) => Self::from_big(value.negated()),
        }
    }

    /// Значение, если оно помещается в signed-64.
    #[inline(always)]
    pub fn to_i64(&self) -> Option<i64> {
        match &self.0 {
            Repr::Small(value) => Some(*value),
            Repr::Big(_) => None,
        }
    }

    /// −1, 0 или 1.
    pub fn signum(&self) -> i64 {
        match &self.0 {
            Repr::Small(value) => value.signum(),
            Repr::Big(value) => value.sign(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.0 == Repr::Small(0)
    }

    pub fn is_negative(&self) -> bool {
        self.signum() < 0
    }

    /// Ближайшее binary64 (round half even), как `BigInteger.doubleValue`.
    pub fn to_f64(&self) -> f64 {
        match &self.0 {
            Repr::Small(value) => *value as f64,
            Repr::Big(value) => value
                .to_string()
                .parse()
                .expect("decimal digits are a float"),
        }
    }
}

impl std::fmt::Display for IntValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Repr::Small(value) => value.fmt(f),
            Repr::Big(value) => value.fmt(f),
        }
    }
}
