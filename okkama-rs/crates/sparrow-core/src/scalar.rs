//! Scalar text forms shared with the previous engine (byte-identical output).

/// Shortest round-trip decimal; scientific outside 1e-4..1e6, as Sparrow prints.
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

/// Exact semantics of a float binary operation, for the slow path of machine
/// code (which handles finite results inline).
pub(crate) fn float_binary(op: u64, left: f64, right: f64) -> Result<f64, &'static str> {
    if right == 0.0 {
        match op {
            FLOAT_DIV => return Err("Division by zero"),
            FLOAT_MOD => return Err("Modulo by zero"),
            _ => {}
        }
    }
    let value = match op {
        FLOAT_ADD => left + right,
        FLOAT_SUB => left - right,
        FLOAT_MUL => left * right,
        FLOAT_DIV => left / right,
        FLOAT_MOD => left % right,
        _ => left.powf(right),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Float value must contain a finite number")
    }
}

pub(crate) const FLOAT_ADD: u64 = 0;
pub(crate) const FLOAT_SUB: u64 = 1;
pub(crate) const FLOAT_MUL: u64 = 2;
pub(crate) const FLOAT_DIV: u64 = 3;
pub(crate) const FLOAT_MOD: u64 = 4;
pub(crate) const FLOAT_POW: u64 = 5;
