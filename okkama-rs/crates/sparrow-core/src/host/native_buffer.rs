//! Mutable byte storage lives entirely in Rust. Front reads do not shift the
//! unread suffix; snapshots are immutable Okkama arrays.
use crate::host::*;
use okkama_runtime_abi::{Native, NativeFailure};
use std::collections::VecDeque;
#[derive(Debug)]
pub(crate) struct Buffer {
    bytes: VecDeque<u8>,
    closed: bool,
}
pub(crate) fn handles(native: Native) -> bool {
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
impl Heap {
    fn buffer_failure(
        &mut self,
        native: Native,
        code: &str,
        message: &str,
    ) -> Result<Outcome, Error> {
        let payload = self.native_failure_new(NativeFailure {
            domain: "core.buffer".into(),
            code: code.into(),
            operation: native.symbol().into(),
            message: message.into(),
        })?;
        self.failure_new(crate::host::FailureKind::Native(payload))
            .map(Outcome::Failure)
    }
    pub(crate) fn native_buffer(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        use Native::*;
        if native == BufferStateCreate {
            let bytes = self.native_bytes(args[0])?.into();
            return Ok(Outcome::Value(Slot {
                tag: HANDLE,
                bits: self.handles.allocate(Handle::Buffer(Buffer {
                    bytes,
                    closed: false,
                }))?,
            }));
        }
        if args[0].kind() != HANDLE {
            return Err(Error("expected native BufferState handle".into()));
        }
        // Preserve provider ordering: append validates its bytes before checking
        // closed state; read checks closed state before its positive limit.
        let append = if native == BufferStateAppend {
            Some(self.native_bytes(args[1])?)
        } else {
            None
        };
        let Handle::Buffer(buffer) = self.handles.get(args[0].bits)? else {
            return Err(Error("expected native BufferState handle".into()));
        };
        if buffer.closed && !matches!(native, BufferStateClosed | BufferStateClose) {
            return self.buffer_failure(native, "already_closed", "Byte buffer is already closed");
        }
        let count = if native == BufferStateRead {
            match self.integer(args[1])? {
                Integer::Small(n) if n > 0 => (n as usize).min(buffer.bytes.len()),
                Integer::Big(n) if n.sign() == num_bigint::Sign::Plus => buffer.bytes.len(),
                _ => {
                    return self.buffer_failure(
                        native,
                        "invalid_input",
                        "Buffer read limit must be positive",
                    );
                }
            }
        } else {
            0
        };
        let Handle::Buffer(buffer) = self.handles.get_mut(args[0].bits)? else {
            unreachable!()
        };
        match native {
            BufferStateRead => {
                let bytes = buffer.bytes.drain(..count).collect();
                self.native_byte_array(bytes).map(Outcome::Value)
            }
            BufferStateAppend => {
                let added = append.expect("validated append");
                let size = added.len();
                buffer.bytes.extend(added);
                Ok(Outcome::Value(Slot::small(size as i64)))
            }
            BufferStateBytes => {
                let bytes = buffer.bytes.iter().copied().collect();
                self.native_byte_array(bytes).map(Outcome::Value)
            }
            BufferStateSize => Ok(Outcome::Value(Slot::small(buffer.bytes.len() as i64))),
            BufferStateClosed => Ok(Outcome::Value(Slot::boolean(buffer.closed))),
            BufferStateClose => {
                let changed = !buffer.closed;
                buffer.closed = true;
                buffer.bytes = VecDeque::new();
                Ok(Outcome::Value(Slot::boolean(changed)))
            }
            _ => Err(Error("invalid buffer provider".into())),
        }
    }
}
