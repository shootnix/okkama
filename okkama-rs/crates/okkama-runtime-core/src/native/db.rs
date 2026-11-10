//! Immutable database cells. External DbValue protocols are checked by stdlib
//! once; native driver results enter this representation directly.
use super::{Error, support::*};
use crate::{IntValue, Value, abi::Native, bignum::Big};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Clone, Debug)]
pub enum Cell {
    Null,
    Boolean(bool),
    Integer(IntValue),
    Floating(f64),
    Decimal(Rc<str>),
    Text(Rc<str>),
    Bytes(Rc<[u8]>),
}
impl Cell {
    pub fn kind(&self) -> i64 {
        match self {
            Self::Null => 0,
            Self::Boolean(_) => 1,
            Self::Integer(_) => 2,
            Self::Floating(_) => 3,
            Self::Decimal(_) => 4,
            Self::Text(_) => 5,
            Self::Bytes(_) => 6,
        }
    }
}
pub fn cell_value(cell: Cell) -> Value {
    new_handle(cell)
}
#[derive(Default)]
struct Capture {
    kind: i64,
    started: bool,
    faulted: bool,
    poisoned: bool,
    sealed: bool,
    value: Option<Cell>,
}
struct Schema(HashMap<String, i64>);
fn invalid(n: Native) -> Error {
    failure(
        "db",
        "invalid_input",
        n,
        "invalid database cell or projection",
    )
}
pub fn execute(n: Native, a: &[Value]) -> Result<Value, Error> {
    let name = n.name();
    if name == "db_capture" {
        return Ok(new_handle(RefCell::new(Capture {
            kind: int(&a[0])?
                .to_i64()
                .filter(|v| (0..=6).contains(v))
                .ok_or_else(|| invalid(n))?,
            ..Capture::default()
        })));
    }
    if name.starts_with("db_capture_") {
        let mut c = handle::<RefCell<Capture>>(&a[0])?.borrow_mut();
        return match name {
            "db_capture_emit" => {
                if c.sealed {
                    Ok(int_value(1))
                } else if c.faulted || c.started || int(&a[1])?.to_i64() != Some(c.kind) {
                    c.faulted = true;
                    c.poisoned = true;
                    Ok(int_value(2))
                } else {
                    c.started = true;
                    c.faulted = true;
                    Ok(int_value(0))
                }
            }
            "db_capture_publish" => {
                let cell = handle::<Cell>(&a[1])?;
                let valid = !c.sealed
                    && !c.poisoned
                    && c.started
                    && c.value.is_none()
                    && c.kind == cell.kind();
                if valid {
                    c.value = Some(cell.clone());
                    c.faulted = false;
                } else if !c.sealed {
                    c.faulted = true;
                }
                Ok(Value::Bool(valid))
            }
            "db_capture_complete" => {
                let valid = c.started && !c.faulted && c.value.is_some();
                if valid && boolean(&a[1])? {
                    c.sealed = true;
                }
                Ok(Value::Bool(valid))
            }
            "db_capture_value" => {
                if c.faulted {
                    return Err(invalid(n));
                }
                c.value.clone().map(cell_value).ok_or_else(|| invalid(n))
            }
            _ => Err(invalid(n)),
        };
    }

    let cell = match name {
        "db_null" => Some(Cell::Null),
        "db_boolean" => Some(Cell::Boolean(boolean(&a[0])?)),
        "db_integer" => {
            let s = string(&a[0])?;
            if s.len() > 4096 {
                return Err(invalid(n));
            }
            Some(Cell::Integer(IntValue::from_big(
                Big::parse(s).ok_or_else(|| invalid(n))?,
            )))
        }
        "db_floating" => {
            let f = float(&a[0])?;
            if !f.is_finite() {
                return Err(invalid(n));
            }
            Some(Cell::Floating(f))
        }
        "db_decimal" => {
            let s = string(&a[0])?;
            if s.len() > 4096 || super::numbers::components(s).is_none() {
                return Err(invalid(n));
            }
            Some(Cell::Decimal(Rc::from(s)))
        }
        "db_text" => {
            let s = string(&a[0])?;
            if s.len() > 8388608 {
                return Err(invalid(n));
            }
            Some(Cell::Text(Rc::from(s)))
        }
        "db_bytes" => {
            let b = bytes(&a[0])?;
            if b.len() > 8388608 {
                return Err(invalid(n));
            }
            Some(Cell::Bytes(b.into()))
        }
        _ => None,
    };
    if let Some(c) = cell {
        return Ok(cell_value(c));
    }
    if name == "db_schema" {
        let names = strings(&a[0])?;
        if names.len() > 1024 || names.iter().any(|s| s.len() > 4096) {
            return Err(invalid(n));
        }
        let mut positions = HashMap::new();
        for (i, name) in names.into_iter().enumerate() {
            positions
                .entry(name)
                .and_modify(|p| *p = -2)
                .or_insert(i as i64);
        }
        return Ok(new_handle(Schema(positions)));
    }
    if name == "db_position" {
        return Ok(int_value(
            *handle::<Schema>(&a[0])?
                .0
                .get(string(&a[1])?)
                .unwrap_or(&-1),
        ));
    }
    if name == "db_cells_bytes" {
        let mut size = 0i64;
        for v in items(&a[0])?.iter() {
            size += match handle::<Cell>(v)? {
                Cell::Null => 0,
                Cell::Boolean(_) => 1,
                Cell::Integer(v) => v.to_string().len(),
                Cell::Floating(v) => crate::float::text(*v).len(),
                Cell::Decimal(v) | Cell::Text(v) => v.len(),
                Cell::Bytes(v) => v.len(),
            } as i64;
            if size > 8388608 {
                return Ok(int_value(size));
            }
        }
        return Ok(int_value(size));
    }
    let c = handle::<Cell>(&a[0])?;
    match (name, c) {
        ("db_kind", _) => Ok(int_value(c.kind())),
        ("db_boolean_value", Cell::Boolean(v)) => Ok(Value::Bool(*v)),
        ("db_integer_value", Cell::Integer(v)) => Ok(text(v.to_string())),
        ("db_int_valid", Cell::Integer(v)) => Ok(Value::Bool(v.to_i64().is_some())),
        ("db_int_valid", _) => Ok(Value::Bool(false)),
        ("db_int_value", Cell::Integer(v)) => v.to_i64().map(int_value).ok_or_else(|| invalid(n)),
        ("db_floating_value", Cell::Floating(v)) => Ok(Value::Float(*v)),
        ("db_decimal_value", Cell::Decimal(v)) | ("db_text_value", Cell::Text(v)) => {
            Ok(Value::Str(v.clone()))
        }
        ("db_bytes_value", Cell::Bytes(v)) => Ok(byte_array(v.iter().copied())),
        _ => Err(invalid(n)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_integer_bounds_and_invalid_spelling() {
        for (s, valid) in [
            ("-9223372036854775808", true),
            ("9223372036854775807", true),
            ("-9223372036854775809", false),
            ("9223372036854775808", false),
        ] {
            let c = execute(Native::DbInteger, &[text(s)]).unwrap();
            assert_eq!(
                execute(Native::DbIntValid, std::slice::from_ref(&c))
                    .unwrap()
                    .as_bool()
                    .unwrap(),
                valid
            );
            assert_eq!(execute(Native::DbIntValue, &[c]).is_ok(), valid);
        }
        for s in ["", "+1", "1.0", " 2"] {
            assert!(execute(Native::DbInteger, &[text(s)]).is_err());
        }
    }
    #[test]
    fn schema_retains_ambiguity_and_case() {
        let schema = execute(Native::DbSchema, &[string_array(["x", "X", "x"])]).unwrap();
        for (name, index) in [("x", -2), ("X", 1), ("absent", -1)] {
            assert_eq!(
                execute(Native::DbPosition, &[schema.clone(), text(name)])
                    .unwrap()
                    .as_int()
                    .unwrap()
                    .to_i64(),
                Some(index)
            );
        }
    }
    #[test]
    fn capture_seals_and_rejects_duplicate_events() {
        let capture = execute(Native::DbCapture, &[int_value(5)]).unwrap();
        let emit = || {
            execute(Native::DbCaptureEmit, &[capture.clone(), int_value(5)])
                .unwrap()
                .as_int()
                .unwrap()
                .to_i64()
                .unwrap()
        };
        assert_eq!(emit(), 0);
        let c = execute(Native::DbText, &[text("owned")]).unwrap();
        assert!(
            execute(Native::DbCapturePublish, &[capture.clone(), c])
                .unwrap()
                .as_bool()
                .unwrap()
        );
        assert!(
            execute(
                Native::DbCaptureComplete,
                &[capture.clone(), Value::Bool(true)]
            )
            .unwrap()
            .as_bool()
            .unwrap()
        );
        assert_eq!(emit(), 1);
        let saved = execute(Native::DbCaptureValue, &[capture]).unwrap();
        assert_eq!(
            execute(Native::DbTextValue, &[saved])
                .unwrap()
                .as_string()
                .unwrap(),
            "owned"
        );
        let capture = execute(Native::DbCapture, &[int_value(0)]).unwrap();
        for expected in [0, 2] {
            assert_eq!(
                execute(Native::DbCaptureEmit, &[capture.clone(), int_value(0)])
                    .unwrap()
                    .as_int()
                    .unwrap()
                    .to_i64(),
                Some(expected)
            );
        }
        assert!(
            !execute(Native::DbCaptureComplete, &[capture, Value::Bool(true)])
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }
}
