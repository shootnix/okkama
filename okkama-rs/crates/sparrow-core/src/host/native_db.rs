//! Shared immutable database cells. Driver results own Rust data directly;
//! captures and schemas never retain VM objects or cursor lifetimes.
use crate::host::*;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use okkama_runtime_abi::{Native, NativeFailure};
use std::{collections::HashMap, rc::Rc};
#[derive(Debug, Clone)]
pub(crate) enum Cell {
    Null,
    Boolean(bool),
    Integer(Rc<BigInt>),
    Floating(f64),
    Decimal(Rc<str>),
    Text(Rc<str>),
    Bytes(Rc<[u8]>),
}
impl Cell {
    pub(crate) fn kind(&self) -> i64 {
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
    fn size(&self) -> usize {
        match self {
            Self::Null => 0,
            Self::Boolean(_) => 1,
            Self::Integer(n) => n.to_string().len(),
            Self::Floating(n) => crate::scalar::float_text(*n).len(),
            Self::Decimal(s) | Self::Text(s) => s.len(),
            Self::Bytes(b) => b.len(),
        }
    }
}
#[derive(Debug, Default)]
pub(crate) struct Capture {
    kind: i64,
    started: bool,
    faulted: bool,
    poisoned: bool,
    sealed: bool,
    value: Option<Cell>,
}
#[derive(Debug)]
pub(crate) enum DbHandle {
    Cell(Cell),
    Capture(Capture),
    Schema(HashMap<Rc<str>, i64, HandleBuild>),
}
pub(crate) fn handles(native: Native) -> bool {
    native.symbol().starts_with("runtime/db@1::")
}
fn invalid(native: Native) -> ProviderError {
    ProviderError::Native(NativeFailure {
        domain: "core.db".into(),
        code: "invalid_input".into(),
        operation: native.symbol().into(),
        message: "invalid database cell or projection".into(),
    })
}
impl Heap {
    fn db_handle(&self, value: Slot) -> Result<&DbHandle, Error> {
        if value.kind() != HANDLE {
            return Err(Error("expected database handle".into()));
        }
        match self.handles.get(value.bits)? {
            Handle::Db(handle) => Ok(handle),
            _ => Err(Error("expected database handle".into())),
        }
    }
    pub(crate) fn db_cell(&self, value: Slot) -> Result<&Cell, Error> {
        match self.db_handle(value)? {
            DbHandle::Cell(cell) => Ok(cell),
            _ => Err(Error("expected database cell".into())),
        }
    }
    fn db_new(&mut self, state: DbHandle) -> Result<Slot, Error> {
        Ok(Slot {
            tag: HANDLE,
            bits: self.handles.allocate(Handle::Db(state))?,
        })
    }
    pub(crate) fn db_cell_new(&mut self, cell: Cell) -> Result<Slot, Error> {
        self.db_new(DbHandle::Cell(cell))
    }
    fn db_string(&self, value: Slot) -> Result<Rc<str>, Error> {
        Ok(self.text(value)?.into())
    }
    pub(crate) fn native_db(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.db_execute(n, a) {
            Ok(value) => Ok(Outcome::Value(value)),
            Err(ProviderError::Engine(error)) => Err(error),
            Err(ProviderError::Native(error)) => {
                let payload = self.native_failure_new(error)?;
                self.failure_new(crate::host::FailureKind::Native(payload))
                    .map(Outcome::Failure)
            }
        }
    }
    fn db_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        if n == DbCapture {
            let kind = self
                .int64(a[0])?
                .filter(|n| (0..=6).contains(n))
                .ok_or_else(|| invalid(n))?;
            return Ok(self.db_new(DbHandle::Capture(Capture {
                kind,
                ..Capture::default()
            }))?);
        }
        if matches!(
            n,
            DbCaptureEmit | DbCapturePublish | DbCaptureComplete | DbCaptureValue
        ) {
            // Read and validate arguments before borrowing mutable capture state.
            let kind = if n == DbCaptureEmit {
                self.int64(a[1])?
            } else {
                None
            };
            let cell = if n == DbCapturePublish {
                Some(self.db_cell(a[1])?.clone())
            } else {
                None
            };
            if !matches!(self.db_handle(a[0])?, DbHandle::Capture(_)) {
                return Err(invalid(n));
            }
            let Handle::Db(DbHandle::Capture(c)) = self.handles.get_mut(a[0].bits)? else {
                unreachable!()
            };
            return match n {
                DbCaptureEmit => Ok(Slot::small(if c.sealed {
                    1
                } else if c.faulted || c.started || kind != Some(c.kind) {
                    c.faulted = true;
                    c.poisoned = true;
                    2
                } else {
                    c.started = true;
                    c.faulted = true;
                    0
                })),
                DbCapturePublish => {
                    let cell = cell.expect("validated cell");
                    let valid = !c.sealed
                        && !c.poisoned
                        && c.started
                        && c.value.is_none()
                        && c.kind == cell.kind();
                    if valid {
                        c.value = Some(cell);
                        c.faulted = false;
                    } else if !c.sealed {
                        c.faulted = true;
                    }
                    Ok(Slot::boolean(valid))
                }
                DbCaptureComplete => {
                    let valid = c.started && !c.faulted && c.value.is_some();
                    if valid && a[1].bits != 0 {
                        c.sealed = true;
                    }
                    Ok(Slot::boolean(valid))
                }
                DbCaptureValue => {
                    if c.faulted {
                        return Err(invalid(n));
                    }
                    let cell = c.value.clone().ok_or_else(|| invalid(n))?;
                    Ok(self.db_cell_new(cell)?)
                }
                _ => unreachable!(),
            };
        }
        let cell = match n {
            DbNull => Some(Cell::Null),
            DbBoolean => Some(Cell::Boolean(a[0].bits != 0)),
            DbInteger => {
                let s = self.text(a[0])?;
                let digits = s.strip_prefix('-').unwrap_or(s);
                if s.len() > 4096
                    || digits.is_empty()
                    || !digits.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(invalid(n));
                }
                Some(Cell::Integer(Rc::new(
                    BigInt::parse_bytes(s.as_bytes(), 10).ok_or_else(|| invalid(n))?,
                )))
            }
            DbFloating => {
                let f = f64::from_bits(a[0].bits);
                if !f.is_finite() {
                    return Err(invalid(n));
                }
                Some(Cell::Floating(f))
            }
            DbDecimal | DbText => {
                let s = self.db_string(a[0])?;
                if n == DbDecimal {
                    if s.len() > 4096 || crate::host::native_numbers::components(&s).is_none() {
                        return Err(invalid(n));
                    }
                    Some(Cell::Decimal(s))
                } else {
                    if s.len() > 8388608 {
                        return Err(invalid(n));
                    }
                    Some(Cell::Text(s))
                }
            }
            DbBytes => {
                if self.array_length(a[0])? > 8388608 {
                    return Err(invalid(n));
                }
                Some(Cell::Bytes(self.native_bytes(a[0])?.into()))
            }
            _ => None,
        };
        if let Some(cell) = cell {
            return Ok(self.db_cell_new(cell)?);
        }
        if n == DbSchema {
            let names = self.elements(a[0])?;
            if names.len() > 1024 {
                return Err(invalid(n));
            }
            let mut positions =
                HashMap::with_capacity_and_hasher(names.len(), HandleBuild::default());
            for (i, name) in names.iter().enumerate() {
                let name = self.db_string(*name)?;
                if name.len() > 4096 {
                    return Err(invalid(n));
                }
                positions
                    .entry(name)
                    .and_modify(|p| *p = -2)
                    .or_insert(i as i64);
            }
            return Ok(self.db_new(DbHandle::Schema(positions))?);
        }
        if n == DbPosition {
            let DbHandle::Schema(schema) = self.db_handle(a[0])? else {
                return Err(invalid(n));
            };
            return Ok(Slot::small(*schema.get(self.text(a[1])?).unwrap_or(&-1)));
        }
        if n == DbCellsBytes {
            let mut size = 0;
            for value in self.elements(a[0])?.iter() {
                size += self.db_cell(*value)?.size();
                if size > 8388608 {
                    break;
                }
            }
            return Ok(Slot::small(size as i64));
        }
        let cell = self.db_cell(a[0])?;
        let value = match (n, cell) {
            (DbKind, c) => Slot::small(c.kind()),
            (DbBooleanValue, Cell::Boolean(v)) => Slot::boolean(*v),
            (DbIntegerValue, Cell::Integer(v)) => {
                let s = v.to_string();
                self.allocate_text(s)?
            }
            (DbIntValid, Cell::Integer(v)) => Slot::boolean(v.to_i64().is_some()),
            (DbIntValid, _) => Slot::boolean(false),
            (DbIntValue, Cell::Integer(v)) => Slot::small(v.to_i64().ok_or_else(|| invalid(n))?),
            (DbFloatingValue, Cell::Floating(v)) => Slot {
                tag: FLOAT,
                bits: v.to_bits(),
            },
            (DbTextValue, Cell::Text(v)) | (DbDecimalValue, Cell::Decimal(v)) => {
                let text = v.to_string();
                self.allocate_text(text)?
            }
            (DbBytesValue, Cell::Bytes(v)) => {
                let bytes = v.to_vec();
                self.native_byte_array(bytes)?
            }
            _ => return Err(invalid(n)),
        };
        Ok(value)
    }
}

