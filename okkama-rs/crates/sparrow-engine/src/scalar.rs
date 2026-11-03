//! Scalar contracts independent of the legacy runtime.
use crate::{
    Error,
    value::{FLOAT, Slot},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Int,
    Bool,
    Byte,
    Char,
    Float,
    Rat,
    Range,
    Any,
    Handle(u32),
    Failure,
    Region,
    Future(Option<u32>),
    Function(u32),
    Role(u32),
    Cap(u32),
    Successor(u32),
    Enum(u32),
    Map(u32, u32),
    MapBuilder(u32, u32),
    Str,
    Array(u32),
    Builder(u32),
    Object(u32),
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum FloatBinary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Power,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum FloatUnary {
    Negate,
    Absolute,
    Sqrt,
}

pub(crate) fn finite(value: f64) -> Result<Slot, Error> {
    if !value.is_finite() {
        return Err(Error("Float value must contain a finite number".into()));
    }
    Ok(Slot {
        bits: value.to_bits(),
        tag: FLOAT,
    })
}
pub(crate) fn float_binary(op: FloatBinary, left: Slot, right: Slot) -> Result<Slot, Error> {
    let (l, r) = (f64::from_bits(left.bits), f64::from_bits(right.bits));
    if r == 0.0 {
        match op {
            FloatBinary::Divide => return Err(Error("Division by zero".into())),
            FloatBinary::Remainder => return Err(Error("Modulo by zero".into())),
            _ => {}
        }
    }
    finite(match op {
        FloatBinary::Add => l + r,
        FloatBinary::Subtract => l - r,
        FloatBinary::Multiply => l * r,
        FloatBinary::Divide => l / r,
        FloatBinary::Remainder => l % r,
        FloatBinary::Power => l.powf(r),
    })
}
pub(crate) fn float_unary(op: FloatUnary, value: Slot) -> Result<Slot, Error> {
    let n = f64::from_bits(value.bits);
    if matches!(op, FloatUnary::Sqrt) && n < 0.0 {
        return Err(Error(format!("Can't take sqrt of {}", float_text(n))));
    }
    finite(match op {
        FloatUnary::Negate => -n,
        FloatUnary::Absolute => n.abs(),
        FloatUnary::Sqrt => n.sqrt(),
    })
}

/// Go-style shortest 'g' formatting: fixed exponents -4..5, signed zero.
pub(crate) fn float_text(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific format");
    let exponent: i32 = exponent.parse().expect("decimal exponent");
    if (-4..6).contains(&exponent) {
        value.to_string()
    } else {
        format!(
            "{mantissa}e{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        )
    }
}
