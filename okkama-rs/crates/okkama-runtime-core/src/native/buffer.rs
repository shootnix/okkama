//! `runtime/buffer`: изменяемый байтовый буфер `BufferState` — перенос
//! `TextNatives` JVM-рантайма (те же отказы `core.buffer`).

use std::cell::RefCell;
use std::rc::Rc;

use super::Error;
use crate::{IntValue, Value, abi::Native, abi::NativeFailure, heap::ArrayHeap};

#[derive(Debug, Default)]
pub struct Buffer {
    bytes: Vec<u8>,
    closed: bool,
}

fn failure(native: Native, code: &str, message: &str) -> Error {
    Error::Failure(NativeFailure {
        domain: "core.buffer".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: message.into(),
    })
}

fn bytes(value: &Value, arrays: &ArrayHeap) -> Result<Vec<u8>, Error> {
    Ok(arrays
        .elements(value)?
        .iter()
        .map(Value::as_byte)
        .collect::<Result<Vec<_>, _>>()?)
}

fn state(value: &Value) -> Result<&RefCell<Buffer>, Error> {
    super::support::handle::<RefCell<Buffer>>(value)
}

/// Открытый буфер; закрытый — `already_closed` (кроме `closed`/`close`).
fn open(native: Native, value: &Value) -> Result<&RefCell<Buffer>, Error> {
    let buffer = state(value)?;
    if buffer.borrow().closed {
        return Err(failure(
            native,
            "already_closed",
            "Byte buffer is already closed",
        ));
    }
    Ok(buffer)
}

pub fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        BufferStateCreate
            | BufferStateRead
            | BufferStateAppend
            | BufferStateBytes
            | BufferStateSize
            | BufferStateClosed
            | BufferStateClose
    )
}

pub fn execute(native: Native, args: &[Value], arrays: &mut ArrayHeap) -> Result<Value, Error> {
    use Native::*;
    let int = |value: usize| Value::Int(IntValue::from_u128(value as u128));
    Ok(match native {
        BufferStateCreate => {
            let initial = bytes(&args[0], arrays)?;
            Value::Handle(Rc::new(RefCell::new(Buffer {
                bytes: initial,
                closed: false,
            })))
        }
        BufferStateRead => {
            let buffer = open(native, &args[0])?;
            let limit = args[1].as_int()?;
            if !matches!(limit.to_i64(), Some(value) if value > 0) && !is_positive_big(limit) {
                return Err(failure(
                    native,
                    "invalid_input",
                    "Buffer read limit must be positive",
                ));
            }
            let mut buffer = buffer.borrow_mut();
            let count = limit.to_i64().map_or(buffer.bytes.len(), |limit| {
                (limit as usize).min(buffer.bytes.len())
            });
            let taken: Vec<u8> = buffer.bytes.drain(..count).collect();
            arrays.allocate(taken.into_iter().map(Value::Byte).collect())
        }
        BufferStateAppend => {
            let added = bytes(&args[1], arrays)?;
            let buffer = open(native, &args[0])?;
            let size = added.len();
            buffer.borrow_mut().bytes.extend(added);
            int(size)
        }
        BufferStateBytes => {
            let snapshot = open(native, &args[0])?.borrow().bytes.clone();
            arrays.allocate(snapshot.into_iter().map(Value::Byte).collect())
        }
        BufferStateSize => int(open(native, &args[0])?.borrow().bytes.len()),
        BufferStateClosed => Value::Bool(state(&args[0])?.borrow().closed),
        BufferStateClose => {
            let mut buffer = state(&args[0])?.borrow_mut();
            if buffer.closed {
                Value::Bool(false)
            } else {
                buffer.closed = true;
                buffer.bytes = Vec::new();
                Value::Bool(true)
            }
        }
        _ => return Err(Error::InvalidState("not a buffer native".into())),
    })
}

/// Положительное значение вне i64 (больше любого буфера).
fn is_positive_big(value: &IntValue) -> bool {
    value.to_i64().is_none() && !value.is_negative()
}
