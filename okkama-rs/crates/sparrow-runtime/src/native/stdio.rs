//! Raw byte streams. Text encoding and line normalization belong to std.
use super::{Error, Native, NativeFailure};
use crate::{IntValue, Value, heap::ArrayHeap};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
};

#[derive(Debug, Default)]
pub struct State {
    buffered: VecDeque<u8>,
    eof: bool,
}

impl State {
    /// Explicit streams allow deterministic provider tests and future embedding.
    pub fn execute(
        &mut self,
        native: Native,
        args: &[Value],
        arrays: &mut ArrayHeap,
        input: &mut dyn Read,
        output: &mut dyn Write,
        errors: &mut dyn Write,
    ) -> Result<Value, Error> {
        if args.len() != native.parameters().len() {
            return Err(Error::InvalidState(
                "stdio argument count does not match ABI".into(),
            ));
        }
        match native {
            Native::StdoutWrite | Native::StderrWrite => {
                let bytes = arrays
                    .elements(&args[0])?
                    .iter()
                    .map(Value::as_byte)
                    .collect::<Result<Vec<_>, _>>()?;
                if bytes.is_empty() {
                    return Ok(Value::Int(IntValue::from_i64(0)));
                }
                let (stream, message): (&mut dyn Write, _) = if native == Native::StdoutWrite {
                    (output, "Cannot write standard output")
                } else {
                    (errors, "Cannot write standard error output")
                };
                let written = retry(|| stream.write(&bytes))
                    .map_err(|error| io_failure(native, error, message))?;
                if written == 0 {
                    return Err(failure(native, "unexpected", message));
                }
                retry(|| stream.flush()).map_err(|error| io_failure(native, error, message))?;
                Ok(Value::Int(IntValue::from_u128(written as u128)))
            }
            Native::StdinRead => {
                let limit = args[0].as_int()?;
                if limit.signum() <= 0 {
                    return Err(failure(
                        native,
                        "invalid_input",
                        "Standard input read size must be positive",
                    ));
                }
                let count = limit.to_i64().map_or(65536, |n| n.min(65536) as usize);
                if self.buffered.is_empty() && !self.eof {
                    self.fill(input, count, native)?;
                }
                let count = count.min(self.buffered.len());
                Ok(arrays.allocate(self.buffered.drain(..count).map(Value::Byte).collect()))
            }
            Native::StdinReadLine => {
                let mut scanned = 0;
                loop {
                    if let Some(offset) = self
                        .buffered
                        .iter()
                        .skip(scanned)
                        .position(|byte| *byte == b'\n')
                    {
                        let count = scanned + offset + 1;
                        return Ok(arrays
                            .allocate(self.buffered.drain(..count).map(Value::Byte).collect()));
                    }
                    if self.eof {
                        return Ok(
                            arrays.allocate(self.buffered.drain(..).map(Value::Byte).collect())
                        );
                    }
                    scanned = self.buffered.len();
                    self.fill(input, 4096, native)?;
                }
            }
            _ => Err(Error::InvalidState("not a stdio native".into())),
        }
    }

    fn fill(&mut self, input: &mut dyn Read, count: usize, native: Native) -> Result<(), Error> {
        let mut bytes = vec![0; count];
        let count = retry(|| input.read(&mut bytes))
            .map_err(|error| io_failure(native, error, "Cannot read standard input"))?;
        if count == 0 {
            self.eof = true;
        }
        self.buffered.extend(&bytes[..count]);
        Ok(())
    }
}

fn retry<T>(mut action: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match action() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

fn failure(native: Native, code: &str, message: &str) -> Error {
    Error::Failure(NativeFailure {
        domain: "core.stdio".into(),
        code: code.into(),
        operation: native.symbol().into(),
        message: message.into(),
    })
}

fn io_failure(native: Native, error: io::Error, message: &str) -> Error {
    use io::ErrorKind::*;
    let code = match error.kind() {
        NotFound => "not_found",
        PermissionDenied => "permission_denied",
        ConnectionRefused => "connection_refused",
        ConnectionReset | ConnectionAborted => "connection_reset",
        TimedOut => "timed_out",
        AddrInUse => "address_in_use",
        AddrNotAvailable => "address_not_available",
        NotConnected => "not_connected",
        BrokenPipe => "broken_pipe",
        WouldBlock => "would_block",
        Interrupted => "interrupted",
        InvalidInput => "invalid_input",
        Unsupported => "unsupported",
        OutOfMemory => "resource_exhausted",
        AlreadyExists => "already_exists",
        _ => "unexpected",
    };
    failure(native, code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn bytes(heap: &ArrayHeap, value: &Value) -> Vec<u8> {
        heap.elements(value)
            .unwrap()
            .iter()
            .map(|v| v.as_byte().unwrap())
            .collect()
    }

    #[test]
    fn line_and_raw_reads_share_buffer_and_stable_eof() {
        let mut state = State::default();
        let mut heap = ArrayHeap::default();
        let mut input = Cursor::new(b"a\r\nb\0c\nlast".to_vec());
        let mut out = Vec::new();
        let mut err = Vec::new();
        for (native, args, expected) in [
            (Native::StdinReadLine, vec![], &b"a\r\n"[..]),
            (
                Native::StdinRead,
                vec![Value::Int(IntValue::from_i64(2))],
                &b"b\0"[..],
            ),
            (Native::StdinReadLine, vec![], &b"c\n"[..]),
            (Native::StdinReadLine, vec![], &b"last"[..]),
            (Native::StdinReadLine, vec![], &b""[..]),
        ] {
            let value = state
                .execute(native, &args, &mut heap, &mut input, &mut out, &mut err)
                .unwrap();
            assert_eq!(bytes(&heap, &value), expected);
        }
        input.set_position(0);
        let value = state
            .execute(
                Native::StdinRead,
                &[Value::Int(IntValue::from_i64(2))],
                &mut heap,
                &mut input,
                &mut out,
                &mut err,
            )
            .unwrap();
        assert!(bytes(&heap, &value).is_empty());
        assert_eq!(input.position(), 0, "EOF must not read upstream again");
    }

    #[test]
    fn limits_are_positive_exact_and_bound_the_read_allocation() {
        let mut state = State::default();
        let mut heap = ArrayHeap::default();
        let mut input = Cursor::new(vec![42; 70000]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        for limit in ["0", "-1", "-99999999999999999999999999999"] {
            let result = state.execute(
                Native::StdinRead,
                &[Value::Int(IntValue::parse(limit).unwrap())],
                &mut heap,
                &mut input,
                &mut out,
                &mut err,
            );
            assert!(
                matches!(result, Err(Error::Failure(f)) if f.code == "invalid_input" && f.domain == "core.stdio")
            );
            assert_eq!(input.position(), 0);
        }
        let limit = Value::Int(IntValue::parse("99999999999999999999999999999").unwrap());
        let value = state
            .execute(
                Native::StdinRead,
                &[limit],
                &mut heap,
                &mut input,
                &mut out,
                &mut err,
            )
            .unwrap();
        assert_eq!(bytes(&heap, &value).len(), 65536);
    }

    #[derive(Default)]
    struct ShortWriter {
        bytes: Vec<u8>,
        calls: usize,
        flushes: usize,
    }
    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls == 1 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = bytes.len().min(2);
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }
    #[test]
    fn writes_preserve_binary_prefix_counts_and_separate_streams() {
        let mut state = State::default();
        let mut heap = ArrayHeap::default();
        let data = heap.allocate([0, 255, 10, 42].map(Value::Byte).to_vec());
        let mut input = io::empty();
        let mut out = ShortWriter::default();
        let mut err = Vec::new();
        let value = state
            .execute(
                Native::StdoutWrite,
                std::slice::from_ref(&data),
                &mut heap,
                &mut input,
                &mut out,
                &mut err,
            )
            .unwrap();
        assert_eq!(value.as_int().unwrap().to_string(), "2");
        assert_eq!(out.bytes, [0, 255]);
        assert_eq!(out.calls, 2);
        assert_eq!(out.flushes, 1);
        assert!(err.is_empty());
        let value = state
            .execute(
                Native::StderrWrite,
                &[data],
                &mut heap,
                &mut input,
                &mut out,
                &mut err,
            )
            .unwrap();
        assert_eq!(value.as_int().unwrap().to_string(), "4");
        assert_eq!(err, [0, 255, 10, 42]);
    }

    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn broken_output_is_a_structured_provider_failure() {
        let mut state = State::default();
        let mut heap = ArrayHeap::default();
        let data = heap.allocate(vec![Value::Byte(42)]);
        let result = state.execute(
            Native::StdoutWrite,
            &[data],
            &mut heap,
            &mut io::empty(),
            &mut BrokenWriter,
            &mut Vec::new(),
        );
        assert_eq!(
            result.unwrap_err(),
            failure(
                Native::StdoutWrite,
                "broken_pipe",
                "Cannot write standard output"
            )
        );
        let empty = heap.allocate(vec![]);
        assert_eq!(
            state
                .execute(
                    Native::StdoutWrite,
                    &[empty],
                    &mut heap,
                    &mut io::empty(),
                    &mut BrokenWriter,
                    &mut Vec::new()
                )
                .unwrap()
                .as_int()
                .unwrap()
                .to_string(),
            "0"
        );
    }
}
