//! Process worker bridge: only transfer bytes cross into Tokio's process owner.
use crate::{
    Code, Error, Executable, Mode, Program,
    failure::FailureKind,
    future::State,
    value::{Heap, Slot},
    worker_wire::{self, Reader},
};
use std::io::Write;
const FRAME: &[u8; 4] = b"OKW1";
const TRAILER: &[u8; 4] = b"OKL1";
impl Heap {
    pub(crate) fn worker_bind(
        &mut self,
        action: Slot,
        entry: Slot,
        fields: Vec<Slot>,
    ) -> Result<Slot, Error> {
        self.closure(entry)?;
        let (function, captures) = self.closure(action)?;
        let captures = captures.to_vec();
        let action = self.closure_new(function, captures.into_iter())?;
        self.worker_bindings.insert(action.bits, (entry, fields));
        Ok(action)
    }
    pub(crate) fn worker_spawn(
        &mut self,
        code: &Code,
        entry: Slot,
        args: Vec<Slot>,
        direct: bool,
    ) -> Result<Slot, Error> {
        if code.worker_command.is_empty() {
            return Err(Error(
                "process workers require an explicit worker command".into(),
            ));
        }
        let (entry, args) = if direct {
            (entry, args)
        } else {
            let (entry,fields)=self.worker_bindings.get(&entry.bits).ok_or_else(||Error("spawn requires a synchronous bound object action whose receiver fields reconstruct it".into()))?;
            let mut values = fields.clone();
            values.extend(args);
            (*entry, values)
        };
        let (function, captures) = self.closure(entry)?;
        if !captures.is_empty() {
            return Err(Error("worker entry cannot capture VM values".into()));
        }
        let mut request = Vec::new();
        worker_wire::text(&mut request, &self.working_directory()?.to_string_lossy())?;
        worker_wire::text(&mut request, &code.function(function).symbol)?;
        worker_wire::count(&mut request, args.len())?;
        for value in args {
            worker_wire::encode(self, &mut request, value)?;
        }
        let future = self.future_with_state(State::Worker)?;
        self.futures.get_mut(future.bits)?.worker = Some(crate::worker_process::Request::new(
            code.worker_command.clone(),
            request,
            self.environment.clone(),
        ));
        Ok(future)
    }
    pub(crate) fn worker_settle(&mut self, future: Slot) -> Result<(), Error> {
        let request = self
            .future(future)?
            .worker
            .as_ref()
            .ok_or_else(|| Error("missing worker request".into()))?;
        let result = request
            .result()
            .ok_or_else(|| Error("worker has not completed".into()))?;
        let outcome = match result {
            Err(failure) => Err(match &*failure {
                crate::worker_process::Failure::Cancelled => {
                    self.failure_new(FailureKind::Cancelled)?
                }
                crate::worker_process::Failure::Process(message) => self.failure_new(
                    FailureKind::Program(format!("cannot execute spawn worker: {message}")),
                )?,
            }),
            Ok(output) => match self.worker_output(&output) {
                Ok(value) => Ok(value),
                Err(error) => Err(self.failure_new(FailureKind::Program(error.0))?),
            },
        };
        self.future_state(future, State::Ready(outcome))
    }
    fn worker_output(&mut self, output: &crate::worker_process::Output) -> Result<Slot, Error> {
        if !output.success {
            return Err(Error(if output.stderr.is_empty() {
                format!(
                    "spawn worker exited with status {}",
                    output.code.unwrap_or(1)
                )
            } else {
                String::from_utf8_lossy(&output.stderr).trim().into()
            }));
        }
        let raw = &output.stdout;
        if raw.len() < 17 || !raw.ends_with(TRAILER) {
            return Err(Error("invalid worker response trailer".into()));
        }
        let end = raw.len() - 12;
        let length = u64::from_be_bytes(raw[end..end + 8].try_into().expect("eight bytes"));
        let start = usize::try_from(length)
            .ok()
            .and_then(|length| end.checked_sub(length))
            .ok_or_else(|| Error("invalid worker response length".into()))?;
        if !raw[start..end].starts_with(FRAME) {
            return Err(Error("invalid worker response frame".into()));
        }
        std::io::stdout()
            .write_all(&raw[..start])
            .map_err(|e| Error(e.to_string()))?;
        std::io::stdout()
            .flush()
            .map_err(|e| Error(e.to_string()))?;
        std::io::stderr()
            .write_all(&output.stderr)
            .map_err(|e| Error(e.to_string()))?;
        let mut reader = Reader::new(&raw[start + 4..end]);
        match reader.take(1)?[0] {
            1 => {
                let value = reader.value(self)?;
                reader.finish()?;
                Ok(value)
            }
            0 => {
                let message = reader.text()?.to_owned();
                reader.finish()?;
                Err(Error(message))
            }
            _ => Err(Error("invalid worker response status".into())),
        }
    }
}

impl Executable {
    /// Supply exact host-split program arguments before creating sessions.
    pub fn with_arguments(mut self, arguments: Vec<std::ffi::OsString>) -> Result<Self, Error> {
        let code = std::rc::Rc::get_mut(&mut self.code)
            .ok_or_else(|| Error("configure arguments before starting a session".into()))?;
        code.arguments = arguments
            .into_iter()
            .map(|arg| {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    arg.into_vec()
                }
                #[cfg(not(unix))]
                {
                    arg.to_string_lossy().as_bytes().to_vec()
                }
            })
            .collect();
        Ok(self)
    }
    /// Override the executable and SPB command used for isolated process workers.
    pub fn with_worker_command(mut self, command: Vec<std::ffi::OsString>) -> Result<Self, Error> {
        let code = std::rc::Rc::get_mut(&mut self.code)
            .ok_or_else(|| Error("configure workers before starting a session".into()))?;
        if command.is_empty() {
            return Err(Error("empty worker command".into()));
        }
        code.worker_command = command;
        Ok(self)
    }
    /// Execute a framed worker request in the child process, without running main.
    pub fn worker_response(
        program: &Program,
        request: &[u8],
        mode: Mode,
    ) -> Result<Vec<u8>, Error> {
        Self::worker_response_configured(program, request, mode, None)
    }
    /// Execute a request with an explicit command for nested process workers.
    pub fn worker_response_with_command(
        program: &Program,
        request: &[u8],
        mode: Mode,
        command: Vec<std::ffi::OsString>,
    ) -> Result<Vec<u8>, Error> {
        Self::worker_response_configured(program, request, mode, Some(command))
    }
    fn worker_response_configured(
        program: &Program,
        request: &[u8],
        mode: Mode,
        command: Option<Vec<std::ffi::OsString>>,
    ) -> Result<Vec<u8>, Error> {
        let outcome = (|| {
            let mut reader = Reader::new(request);
            let cwd = reader.text()?;
            let target = reader.text()?;
            let entry = program
                .functions
                .iter()
                .position(|f| f.symbol == target)
                .ok_or_else(|| Error("unknown worker entry".into()))?;
            let mut code = Self::load(program, mode, entry)?;
            if let Some(command) = command {
                code = code.with_worker_command(command)?;
            }
            let mut session = code.start();
            let count = reader.count()?;
            if count != program.functions[entry].params.len() {
                return Err(Error("worker argument count mismatch".into()));
            }
            session.worker_arguments(&mut reader)?;
            reader.finish()?;
            session.worker_directory(cwd)?;
            let host = crate::AsyncHost::new()?;
            host.block_on(session.worker_result())?
        })();
        let mut response = FRAME.to_vec();
        match outcome {
            Ok(value) => {
                response.push(1);
                response.extend(value);
            }
            Err(error) => {
                response.push(0);
                worker_wire::text(&mut response, &error.0)?;
            }
        }
        let length = response.len() as u64;
        response.extend_from_slice(&length.to_be_bytes());
        response.extend_from_slice(TRAILER);
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedding_requires_explicit_worker_command_before_creating_a_request() {
        let program =
            okkama_sparrow::assemble(include_str!("../tests/fixtures/async-worker.asm")).unwrap();
        let code = Executable::new(&program, Mode::Cold).unwrap();
        assert!(code.code.worker_command.is_empty());
        let mut heap = Heap::new(&[], &[]);
        let error = heap
            .worker_spawn(&code.code, Slot::default(), vec![], true)
            .unwrap_err();
        assert_eq!(
            error.0,
            "process workers require an explicit worker command"
        );
        assert_eq!(heap.futures.allocations, 0);
        let configured = code
            .with_worker_command(vec!["explicit-host".into(), "module.spb".into()])
            .unwrap();
        assert_eq!(configured.code.worker_command[0], "explicit-host");
    }

    #[test]
    fn bound_actions_trace_transfer_fields_and_worker_entry_without_retaining_dead_cycles() {
        let mut heap = Heap::new(&[], &[]);
        let action = heap.closure_new(0, std::iter::empty()).unwrap();
        let entry = heap.closure_new(1, std::iter::empty()).unwrap();
        let text = heap.allocate_text("transfer field".into()).unwrap();
        let bound = heap.worker_bind(action, entry, vec![text]).unwrap();
        heap.collect([bound].into_iter()).unwrap();
        assert_eq!(heap.closure(entry).unwrap().0, 1);
        assert_eq!(heap.text(text).unwrap(), "transfer field");
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.worker_bindings.is_empty());
        assert!(heap.closure(entry).is_err());
    }
    #[test]
    fn response_trailer_prevents_marker_bytes_inside_values_becoming_a_frame() {
        let mut heap = Heap::new(&[], &[]);
        let value = heap.allocate_text("OKW1\0\0\0\0\0OKL1".into()).unwrap();
        let mut stdout = FRAME.to_vec();
        stdout.push(1);
        worker_wire::encode(&heap, &mut stdout, value).unwrap();
        let length = stdout.len() as u64;
        stdout.extend_from_slice(&length.to_be_bytes());
        stdout.extend_from_slice(TRAILER);
        let mut output = crate::worker_process::Output {
            stdout,
            stderr: vec![],
            success: true,
            code: Some(0),
        };
        let decoded = heap.worker_output(&output).unwrap();
        assert!(heap.equal(value, decoded).unwrap());
        for end in 0..output.stdout.len() {
            let broken = crate::worker_process::Output {
                stdout: output.stdout[..end].to_vec(),
                stderr: vec![],
                success: true,
                code: Some(0),
            };
            assert!(heap.worker_output(&broken).is_err());
        }
        let end = output.stdout.len() - 12;
        output.stdout[end..end + 8].fill(255);
        assert!(heap.worker_output(&output).is_err());
    }
}
