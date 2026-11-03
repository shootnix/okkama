//! The Okkama conversion contract differs from binary-float-to-rational helpers.
use crate::{
    Error,
    scalar::float_text,
    value::{SMALL_RAT, Slot},
};
use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum Unary {
    Negate,
    Absolute,
}

pub(crate) fn parts(slot: Slot) -> Option<(i128, i128)> {
    (slot.tag == SMALL_RAT).then(|| {
        (
            i128::from(slot.bits as u32 as i32),
            i128::from((slot.bits >> 32) as u32),
        )
    })
}
pub(crate) fn pack(mut n: i128, mut d: i128) -> Option<Slot> {
    if d == 0 {
        return None;
    }
    if d < 0 {
        n = n.checked_neg()?;
        d = d.checked_neg()?;
    }
    let gcd = n.unsigned_abs().gcd(&(d as u128)) as i128;
    n /= gcd;
    d /= gcd;
    let n = i32::try_from(n).ok()?;
    let d = u32::try_from(d).ok()?;
    Some(Slot {
        bits: (u64::from(d) << 32) | u64::from(n as u32),
        tag: SMALL_RAT,
    })
}

pub(crate) fn decimal_parts(value: f64) -> (i128, i32) {
    let text = float_text(value);
    let (mantissa, exponent) = text.split_once('e').map_or((text.as_str(), 0), |(m, e)| {
        (m, e.parse::<i32>().expect("float decimal exponent"))
    });
    let fraction = mantissa.split_once('.').map_or(0, |(_, f)| f.len() as i32);
    let digits = mantissa
        .replace('.', "")
        .parse()
        .expect("shortest binary64 decimal fits i128");
    (digits, exponent - fraction)
}

pub(crate) fn decimal_ratio(value: f64) -> BigRational {
    let (digits, exponent) = decimal_parts(value);
    let power = BigInt::from(10).pow(exponent.unsigned_abs());
    if exponent >= 0 {
        BigRational::from_integer(BigInt::from(digits) * power)
    } else {
        BigRational::new(BigInt::from(digits), power)
    }
}

/// DECIMAL128 half-even (34 significant decimal digits), then binary64.
/// This intentionally matches the established Okkama/JVM conversion contract.
pub(crate) fn to_float(value: &BigRational) -> Result<f64, Error> {
    if value.is_zero() {
        return Ok(0.0);
    }
    let n = value.numer().abs();
    let d = value.denom();
    let mut scale = 33 - (n.to_string().len() as i64 - d.to_string().len() as i64);
    let minimum = BigInt::from(10).pow(33);
    let (mut quotient, remainder, divisor) = loop {
        let power = BigInt::from(10).pow(
            u32::try_from(scale.unsigned_abs())
                .map_err(|_| Error("rational conversion exponent too large".into()))?,
        );
        let (numerator, divisor) = if scale >= 0 {
            (&n * power, d.clone())
        } else {
            (n.clone(), d * power)
        };
        let (q, r) = numerator.div_rem(&divisor);
        if q < minimum {
            scale += 1;
        } else if q.to_string().len() > 34 {
            scale -= 1;
        } else {
            break (q, r, divisor);
        }
    };
    let twice = remainder * 2;
    if twice > divisor || (twice == divisor && quotient.is_odd()) {
        quotient += 1;
    }
    let sign = if value.is_negative() { "-" } else { "" };
    let result: f64 = format!("{sign}{quotient}e{}", -scale)
        .parse()
        .map_err(|_| Error("invalid rational conversion".into()))?;
    if !result.is_finite() {
        return Err(Error(
            "Cannot convert rat to float: expected a finite binary64 value".into(),
        ));
    }
    Ok(result)
}
