//! `runtime/input_state` (буфер UTF-8 ввода) и `runtime/process::process_arguments`
//! — часть `Natives` JVM.
use super::support::{array, byte_array, bytes, failure, handle, int, int_value, new_handle};
use super::{Context, Error};
use crate::{Value, abi::Native};
use std::cell::RefCell;

#[derive(Debug, Default)]
struct InputState {
    bytes: Vec<u8>,
    eof: bool,
    closed: bool,
}

fn input<'a>(native: Native, value: &'a Value, allow_closed: bool) -> Result<std::cell::RefMut<'a, InputState>, Error> {
    let state = handle::<RefCell<InputState>>(value)?.borrow_mut();
    if state.closed && !allow_closed {
        return Err(failure(
            "core.input_state",
            "already_closed",
            native,
            "UTF-8 input is already closed",
        ));
    }
    Ok(state)
}

/// Аргументы программы как байты. `OKKAMA_ARGV_HEX` (через запятую, каждый —
/// `x` и hex) сохраняет исходные байты; иначе — UTF-8 аргументов запуска.
fn arguments(context: &Context) -> Vec<Value> {
    if let Ok(hex) = std::env::var("OKKAMA_ARGV_HEX") {
        if hex.is_empty() {
            return Vec::new();
        }
        return hex
            .split(',')
            .map(|part| {
                let part = part.get(1..).unwrap_or("");
                byte_array((0..part.len() / 2).map(|index| {
                    u8::from_str_radix(&part[2 * index..2 * index + 2], 16).unwrap_or(0)
                }))
            })
            .collect();
    }
    context
        .arguments
        .iter()
        .map(|argument| byte_array(argument.bytes()))
        .collect()
}

pub fn execute(native: Native, args: &[Value], context: &Context) -> Result<Value, Error> {
    let operation = native.name();
    match operation {
        "process_arguments" => Ok(array(arguments(context))),
        "input_state_create" => Ok(new_handle(RefCell::new(InputState::default()))),
        "input_state_append" => {
            let data = bytes(&args[1])?;
            let mut state = input(native, &args[0], false)?;
            state.bytes.extend_from_slice(&data);
            Ok(int_value(data.len() as i64))
        }
        "input_state_size" => Ok(int_value(input(native, &args[0], false)?.bytes.len() as i64)),
        "input_state_line_offset" => Ok(int_value(
            input(native, &args[0], false)?
                .bytes
                .iter()
                .position(|b| *b == b'\n')
                .map_or(-1, |index| index as i64),
        )),
        "input_state_take" => {
            let mut state = input(native, &args[0], false)?;
            let count = super::support::small(int(&args[1])?, state.bytes.len() + 1).ok_or_else(|| {
                failure(
                    "core.input_state",
                    "invalid_input",
                    native,
                    "Input-state take size is invalid",
                )
            })?;
            let taken: Vec<u8> = state.bytes.drain(..count).collect();
            Ok(byte_array(taken))
        }
        "input_state_eof" => Ok(Value::Bool(input(native, &args[0], false)?.eof)),
        "input_state_finish" => {
            let mut state = input(native, &args[0], false)?;
            let changed = !state.eof;
            state.eof = true;
            Ok(Value::Bool(changed))
        }
        "input_state_closed" => Ok(Value::Bool(input(native, &args[0], true)?.closed)),
        "input_state_close" => {
            let mut state = input(native, &args[0], true)?;
            if state.closed {
                return Ok(Value::Bool(false));
            }
            state.bytes.clear();
            state.eof = true;
            state.closed = true;
            Ok(Value::Bool(true))
        }
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
