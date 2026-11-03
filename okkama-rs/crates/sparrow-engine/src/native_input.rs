//! Input decorators own only undecoded bytes; standard input retains unread
//! bytes across raw/line calls. Neither state stores a wrapped VM capability.
use crate::{
    Error,
    handle::Handle,
    native::{Outcome, ProviderError},
    value::*,
};
use okkama_runtime_abi::{Native, NativeFailure};
use std::{collections::VecDeque, io::Read};
#[derive(Debug, Default)]
pub(crate) struct Input {
    bytes: VecDeque<u8>,
    eof: bool,
    closed: bool,
}
#[derive(Debug, Default)]
pub(crate) struct Stdin {
    bytes: VecDeque<u8>,
    eof: bool,
}
fn fail(n: Native, code: &str, message: &str) -> ProviderError {
    ProviderError::Native(NativeFailure {
        domain: if matches!(n, Native::StdinRead | Native::StdinReadLine) {
            "core.stdio"
        } else {
            "core.input_state"
        }
        .into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    })
}
pub(crate) fn handles(n: Native) -> bool {
    n.symbol().starts_with("runtime/input_state@1::")
        || matches!(n, Native::StdinRead | Native::StdinReadLine)
}
impl Stdin {
    fn fill(&mut self, n: Native, input: &mut dyn Read, size: usize) -> Result<(), ProviderError> {
        let mut bytes = vec![0; size];
        let size = loop {
            match input.read(&mut bytes) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Ok(size) => break size,
                Err(e) => {
                    return Err(fail(
                        n,
                        crate::native::io_code(e.kind()),
                        "Cannot read standard input",
                    ));
                }
            }
        };
        if size == 0 {
            self.eof = true;
        }
        self.bytes.extend(&bytes[..size]);
        Ok(())
    }
    fn read(
        &mut self,
        n: Native,
        input: &mut dyn Read,
        count: usize,
    ) -> Result<Vec<u8>, ProviderError> {
        if n == Native::StdinRead {
            if self.bytes.is_empty() && !self.eof {
                self.fill(n, input, count)?;
            }
            return Ok(self.bytes.drain(..count.min(self.bytes.len())).collect());
        }
        let mut scanned = 0;
        loop {
            if let Some(at) = self.bytes.iter().skip(scanned).position(|b| *b == b'\n') {
                return Ok(self.bytes.drain(..scanned + at + 1).collect());
            }
            if self.eof {
                return Ok(self.bytes.drain(..).collect());
            }
            scanned = self.bytes.len();
            self.fill(n, input, 4096)?;
        }
    }
}
impl Heap {
    pub(crate) fn native_input(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.input_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                let p = self.native_failure_new(e)?;
                self.failure_new(crate::failure::FailureKind::Native(p))
                    .map(Outcome::Failure)
            }
        }
    }
    fn input_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        if matches!(n, StdinRead | StdinReadLine) {
            let count = if n == StdinRead {
                use num_traits::ToPrimitive;
                let limit = crate::native_file::big(self.integer(a[0])?);
                if limit <= 0.into() {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Standard input read size must be positive",
                    ));
                }
                limit.to_i64().unwrap_or(i64::MAX).min(65536) as usize
            } else {
                0
            };
            let state = self.stdin.get_or_insert_with(Default::default);
            let bytes = state.read(n, &mut std::io::stdin().lock(), count)?;
            return Ok(self.native_byte_array(bytes)?);
        }
        if n == InputStateInputStateCreate {
            return Ok(Slot {
                tag: HANDLE,
                bits: self.handles.allocate(Handle::Input(Input::default()))?,
            });
        }
        let bytes = if n == InputStateInputStateAppend {
            Some(self.native_bytes(a[1])?)
        } else {
            None
        };
        let count = if n == InputStateInputStateTake {
            self.integer(a[1])?.to_i64()
        } else {
            None
        };
        if a[0].tag != HANDLE {
            return Err(Error("expected Utf8InputState".into()).into());
        }
        let Handle::Input(state) = self.handles.get_mut(a[0].bits)? else {
            return Err(Error("expected Utf8InputState".into()).into());
        };
        if n == InputStateInputStateClosed {
            return Ok(Slot::boolean(state.closed));
        }
        if n == InputStateInputStateClose {
            let changed = !state.closed;
            state.bytes.clear();
            state.eof = true;
            state.closed = true;
            return Ok(Slot::boolean(changed));
        }
        if state.closed {
            return Err(fail(n, "already_closed", "UTF-8 input is closed"));
        }
        match n {
            InputStateInputStateAppend => {
                let bytes = bytes.expect("append bytes");
                let len = bytes.len();
                state.bytes.extend(bytes);
                Ok(Slot::small(len as i64))
            }
            InputStateInputStateSize => Ok(Slot::small(state.bytes.len() as i64)),
            InputStateInputStateLineOffset => Ok(Slot::small(
                state
                    .bytes
                    .iter()
                    .position(|b| *b == b'\n')
                    .map_or(-1, |n| n as i64),
            )),
            InputStateInputStateTake => {
                let count = count
                    .filter(|n| *n >= 0 && *n as u64 <= state.bytes.len() as u64)
                    .ok_or_else(|| fail(n, "invalid_input", "Input take size is invalid"))?
                    as usize;
                let bytes = state.bytes.drain(..count).collect();
                Ok(self.native_byte_array(bytes)?)
            }
            InputStateInputStateEof => Ok(Slot::boolean(state.eof)),
            InputStateInputStateFinish => {
                let changed = !state.eof;
                state.eof = true;
                Ok(Slot::boolean(changed))
            }
            _ => Err(Error("unsupported input native".into()).into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_and_line_reads_share_buffer_preserve_crlf_and_have_stable_eof() {
        let mut state = Stdin::default();
        let mut input = std::io::Cursor::new(b"first\r\nsecond\nlast".to_vec());
        assert_eq!(
            state.read(Native::StdinReadLine, &mut input, 0).unwrap(),
            b"first\r\n"
        );
        assert_eq!(
            state.read(Native::StdinRead, &mut input, 3).unwrap(),
            b"sec"
        );
        assert_eq!(
            state.read(Native::StdinReadLine, &mut input, 0).unwrap(),
            b"ond\n"
        );
        assert_eq!(
            state.read(Native::StdinReadLine, &mut input, 0).unwrap(),
            b"last"
        );
        assert!(
            state
                .read(Native::StdinReadLine, &mut input, 0)
                .unwrap()
                .is_empty()
        );
        assert!(
            state
                .read(Native::StdinRead, &mut input, 1)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn input_buffer_partial_unicode_take_bounds_and_lifecycle() {
        let mut h = Heap::new(&[], &[]);
        let state = h
            .input_execute(Native::InputStateInputStateCreate, &[])
            .unwrap();
        let bytes = h.native_byte_array(vec![0xc3]).unwrap();
        h.input_execute(Native::InputStateInputStateAppend, &[state, bytes])
            .unwrap();
        let bytes = h.native_byte_array(vec![0xa9, b'\n']).unwrap();
        h.input_execute(Native::InputStateInputStateAppend, &[state, bytes])
            .unwrap();
        assert_eq!(
            h.input_execute(Native::InputStateInputStateLineOffset, &[state])
                .unwrap()
                .bits,
            2
        );
        assert!(
            h.input_execute(Native::InputStateInputStateTake, &[state, Slot::small(4)])
                .is_err()
        );
        let bytes = h
            .input_execute(Native::InputStateInputStateTake, &[state, Slot::small(3)])
            .unwrap();
        assert_eq!(h.native_bytes(bytes).unwrap(), "é\n".as_bytes());
        assert_eq!(
            h.input_execute(Native::InputStateInputStateFinish, &[state])
                .unwrap()
                .bits,
            1
        );
        assert_eq!(
            h.input_execute(Native::InputStateInputStateFinish, &[state])
                .unwrap()
                .bits,
            0
        );
        h.collect([state].into_iter()).unwrap();
        assert_eq!(
            h.input_execute(Native::InputStateInputStateClose, &[state])
                .unwrap()
                .bits,
            1
        );
        assert_eq!(
            h.input_execute(Native::InputStateInputStateClose, &[state])
                .unwrap()
                .bits,
            0
        );
        assert!(
            h.input_execute(Native::InputStateInputStateSize, &[state])
                .is_err()
        );
    }
}
