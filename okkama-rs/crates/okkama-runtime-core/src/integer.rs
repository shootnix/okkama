//! Shared arbitrary-precision integer arithmetic for IR and Sparrow.

use crate::IntValue;

/// `int` точный (R01, OIR-1 §23): `+`, `-`, `*` и унарный минус не
/// переполняются — `add`, `subtract`, `multiply`, `negate` ниже.
/// Битовые операции и сдвиги работают со словом signed-64; операнд вне его
/// диапазона — ошибка, как в JVM-рантайме (`Rt.word`).
pub fn word(value: &IntValue) -> Result<i64, &'static str> {
    value
        .to_i64()
        .ok_or("Bitwise operand is outside signed-64 range")
}

pub fn subtract(left: &IntValue, right: &IntValue) -> IntValue {
    if let (Some(l), Some(r)) = (left.to_i64(), right.to_i64()) {
        return IntValue::from_i128(i128::from(l) - i128::from(r));
    }
    IntValue::from_big(left.as_big().subtract(&right.as_big()))
}

pub fn negate(value: &IntValue) -> IntValue {
    value.negated()
}

/// Exact ascending range; even the endpoints need not fit a machine integer.
pub fn range(
    start: &IntValue,
    end: &IntValue,
    inclusive: bool,
) -> Result<impl Iterator<Item = IntValue>, &'static str> {
    if compare(start, end).is_gt() {
        return Err("Range start must be less than or equal to range end");
    }
    let mut next = start.clone();
    let end = end.clone();
    let one = IntValue::from_i64(1);
    Ok(std::iter::from_fn(move || {
        let order = compare(&next, &end);
        if order.is_gt() || (!inclusive && order.is_eq()) {
            return None;
        }
        let value = next.clone();
        next = add(&next, &one);
        Some(value)
    }))
}

/// Remainder of truncated division: the sign follows the dividend (OCS D-031).
/// None denotes a zero divisor.
pub fn remainder(left: &IntValue, right: &IntValue) -> Option<IntValue> {
    divide(left, right).map(|(_, rest)| rest)
}

/// Truncated quotient and remainder; None denotes a zero divisor.
pub fn divide(left: &IntValue, right: &IntValue) -> Option<(IntValue, IntValue)> {
    if right.is_zero() {
        return None;
    }
    if let (Some(l), Some(r)) = (left.to_i64(), right.to_i64()) {
        // i64::MIN / -1 не помещается в слово: тогда через i128.
        return Some(match (l.checked_div(r), l.checked_rem(r)) {
            (Some(q), Some(rest)) => (IntValue::from_i64(q), IntValue::from_i64(rest)),
            _ => (
                IntValue::from_i128(i128::from(l) / i128::from(r)),
                IntValue::from_i64(0),
            ),
        });
    }
    let (quotient, rest) = left.as_big().divide(&right.as_big());
    Some((IntValue::from_big(quotient), IntValue::from_big(rest)))
}

/// Non-negative greatest common divisor; gcd(0, 0) = 0.
pub fn gcd(left: &IntValue, right: &IntValue) -> IntValue {
    if let (Some(l), Some(r)) = (left.to_i64(), right.to_i64()) {
        let (mut a, mut b) = (l.unsigned_abs(), r.unsigned_abs());
        while b != 0 {
            (a, b) = (b, a % b);
        }
        return IntValue::from_u128(u128::from(a));
    }
    IntValue::from_big(left.as_big().gcd(&right.as_big()))
}

pub fn compare(left: &IntValue, right: &IntValue) -> std::cmp::Ordering {
    match (left.to_i64(), right.to_i64()) {
        (Some(l), Some(r)) => l.cmp(&r),
        _ => left.as_big().compare(&right.as_big()),
    }
}

pub fn add(left: &IntValue, right: &IntValue) -> IntValue {
    if let (Some(l), Some(r)) = (left.to_i64(), right.to_i64()) {
        return IntValue::from_i128(i128::from(l) + i128::from(r));
    }
    IntValue::from_big(left.as_big().add(&right.as_big()))
}

pub fn multiply(left: &IntValue, right: &IntValue) -> IntValue {
    if let (Some(l), Some(r)) = (left.to_i64(), right.to_i64()) {
        return IntValue::from_i128(i128::from(l) * i128::from(r));
    }
    IntValue::from_big(left.as_big().multiply(&right.as_big()))
}

/// Validate exact slice bounds before converting to host indices.
pub fn slice_range(
    length: usize,
    start: &IntValue,
    end: &IntValue,
) -> Result<std::ops::Range<usize>, &'static str> {
    // Сначала сравнения точных целых. Огромный индекс не должен усекаться
    // до usize или менять приоритет ошибок относительно JVM.
    if start.is_negative() {
        return Err("Slice start must not be negative");
    }
    if compare(start, end).is_gt() {
        return Err("Slice start must be less than or equal to slice end");
    }
    if compare(end, &IntValue::from_u128(length as u128)).is_gt() {
        return Err("Slice end must not exceed length");
    }
    // 0 <= start <= end <= length: оба индекса теперь помещаются в usize.
    let start = usize::try_from(start.to_i64().expect("validated slice index"))
        .expect("validated slice index");
    let end = usize::try_from(end.to_i64().expect("validated slice index"))
        .expect("validated slice index");
    Ok(start..end)
}

/// Bit operations observe the language's signed 64-bit word.
pub fn bit_and(left: &IntValue, right: &IntValue) -> Result<IntValue, &'static str> {
    Ok(IntValue::from_i64(word(left)? & word(right)?))
}
pub fn bit_or(left: &IntValue, right: &IntValue) -> Result<IntValue, &'static str> {
    Ok(IntValue::from_i64(word(left)? | word(right)?))
}
pub fn bit_xor(left: &IntValue, right: &IntValue) -> Result<IntValue, &'static str> {
    Ok(IntValue::from_i64(word(left)? ^ word(right)?))
}
pub fn bit_not(value: &IntValue) -> Result<IntValue, &'static str> {
    Ok(IntValue::from_i64(!word(value)?))
}
/// Counts are checked exactly, before reducing the value to a machine word.
pub fn shift(
    value: &IntValue,
    count: &IntValue,
    left: bool,
    logical: bool,
) -> Result<IntValue, &'static str> {
    let count = count
        .to_i64()
        .filter(|n| (0..64).contains(n))
        .map(|n| n as u32)
        .ok_or("Shift count must be in 0..63")?;
    let value = word(value)?;
    Ok(IntValue::from_i64(if left {
        value << count
    } else if logical {
        ((value as u64) >> count) as i64
    } else {
        value >> count
    }))
}

/// Битовые действия над словом (§16.2); коды — как у `Rt.intBits` JVM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BitOp {
    BitCount = 0,
    LeadingZeros = 1,
    TrailingZeros = 2,
    RotateLeft = 3,
    RotateRight = 4,
    ReverseBits = 5,
    SwapBytes = 6,
}

impl BitOp {
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::BitCount,
            1 => Self::LeadingZeros,
            2 => Self::TrailingZeros,
            3 => Self::RotateLeft,
            4 => Self::RotateRight,
            5 => Self::ReverseBits,
            6 => Self::SwapBytes,
            _ => return None,
        })
    }
}

fn rotation(count: Option<&IntValue>, width: i64) -> Result<u32, &'static str> {
    count
        .and_then(IntValue::to_i64)
        .filter(|n| (0..width).contains(n))
        .map(|n| n as u32)
        .ok_or(if width == 64 {
            "Shift count must be in 0..63"
        } else {
            "Shift count must be in 0..7"
        })
}

/// Действие над словом signed-64; сначала проверяется само значение.
pub fn int_bits(op: BitOp, value: &IntValue, count: Option<&IntValue>) -> Result<IntValue, &'static str> {
    let word = word(value)?;
    Ok(IntValue::from_i64(match op {
        BitOp::BitCount => i64::from(word.count_ones()),
        BitOp::LeadingZeros => i64::from(word.leading_zeros()),
        BitOp::TrailingZeros => i64::from(word.trailing_zeros()),
        BitOp::RotateLeft => word.rotate_left(rotation(count, 64)?),
        BitOp::RotateRight => word.rotate_right(rotation(count, 64)?),
        BitOp::ReverseBits => word.reverse_bits(),
        BitOp::SwapBytes => word.swap_bytes(),
    }))
}

/// Действие над словом 8 бит: счётчики — число, остальные — byte.
pub fn byte_bits(op: BitOp, value: u8, count: Option<&IntValue>) -> Result<u8, &'static str> {
    Ok(match op {
        BitOp::BitCount => value.count_ones() as u8,
        BitOp::LeadingZeros => value.leading_zeros() as u8,
        BitOp::TrailingZeros => value.trailing_zeros() as u8,
        BitOp::RotateLeft => value.rotate_left(rotation(count, 8)?),
        BitOp::RotateRight => value.rotate_right(rotation(count, 8)?),
        BitOp::ReverseBits => value.reverse_bits(),
        // Слово из одного байта: перестановка байтов его не меняет.
        BitOp::SwapBytes => value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_addition_matches_small_integer_arithmetic() {
        for a in -120..=120 {
            for b in -120..=120 {
                assert_eq!(
                    add(&IntValue::from_i64(a), &IntValue::from_i64(b)),
                    IntValue::from_i64(a + b)
                );
                assert_eq!(
                    multiply(&IntValue::from_i64(a), &IntValue::from_i64(b)),
                    IntValue::from_i64(a * b)
                );
                assert_eq!(
                    compare(&IntValue::from_i64(a), &IntValue::from_i64(b)),
                    a.cmp(&b)
                );
                assert_eq!(
                    remainder(&IntValue::from_i64(a), &IntValue::from_i64(b)),
                    if b == 0 {
                        None
                    } else {
                        Some(IntValue::from_i64(a % b))
                    }
                );
            }
        }
    }
}
