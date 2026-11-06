//! Process ABI is a thin typed owner around Tokio children and OS pipes.
use crate::host::{
    Error,
    handle::Handle,
    {Outcome, ProviderError},
    native_io::Output,
    process_host::{self, Captured, Child, Command},
    *,
};
use okkama_runtime_abi::{Native, NativeFailure};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    rc::Rc,
};
#[derive(Debug, Default)]
pub(crate) struct State {
    child: Option<Rc<Child>>,
    input: Option<File>,
    output: Option<File>,
    errors: Option<File>,
    input_closed: bool,
    closed: bool,
    capturing: bool,
}
impl State {
    fn close(&mut self) -> std::io::Result<()> {
        self.closed = true;
        self.input_closed = true;
        self.input.take();
        self.output.take();
        self.errors.take();
        if let Some(child) = &self.child {
            child.close()?;
        }
        Ok(())
    }
    fn cancel(&mut self) {
        self.closed = true;
        self.input_closed = true;
        self.input.take();
        self.output.take();
        self.errors.take();
    }
}
impl Drop for State {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
#[derive(Debug, Default)]
pub(crate) struct Pipeline {
    commands: Vec<Command>,
    children: Vec<Rc<Child>>,
    started: bool,
    closed: bool,
    capturing: bool,
}
impl Drop for Pipeline {
    fn drop(&mut self) {
        for child in &self.children {
            let _ = child.close();
        }
    }
}
#[derive(Debug)]
pub(crate) enum ProcessHandle {
    State(Rc<RefCell<State>>),
    Pipeline(Rc<RefCell<Pipeline>>),
    Captured(Rc<Captured>),
}
impl Output for Captured {
    fn publish(self: Box<Self>, h: &mut Heap) -> Result<Slot, Error> {
        h.process_new(ProcessHandle::Captured(Rc::new(*self)))
    }
}
struct Status(std::process::ExitStatus);
impl Output for Status {
    fn publish(self: Box<Self>, h: &mut Heap) -> Result<Slot, Error> {
        h.process_status(self.0)
    }
}
fn failure(n: Native, code: &str, message: &str) -> NativeFailure {
    NativeFailure {
        domain: "core.process".into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    }
}
fn fail(n: Native, code: &str, message: &str) -> ProviderError {
    ProviderError::Native(failure(n, code, message))
}
fn host(n: Native, error: std::io::Error) -> NativeFailure {
    match crate::host::native_fs::host(n, error) {
        ProviderError::Native(mut e) => {
            e.domain = "core.process".into();
            e
        }
        _ => unreachable!(),
    }
}
pub(crate) fn handles(n: Native) -> bool {
    n.symbol().starts_with("runtime/process@1::")
}
impl Heap {
    fn process_new(&mut self, state: ProcessHandle) -> Result<Slot, Error> {
        Ok(Slot {
            tag: HANDLE,
            bits: self.handles.allocate(Handle::Process(state))?,
        })
    }
    fn process_handle(&self, v: Slot) -> Result<&ProcessHandle, Error> {
        if v.kind() != HANDLE {
            return Err(Error("expected process handle".into()));
        }
        match self.handles.get(v.bits)? {
            Handle::Process(state) => Ok(state),
            _ => Err(Error("expected process handle".into())),
        }
    }
    fn process_state(&self, v: Slot) -> Result<Rc<RefCell<State>>, Error> {
        match self.process_handle(v)? {
            ProcessHandle::State(state) => Ok(state.clone()),
            _ => Err(Error("expected ProcessState".into())),
        }
    }
    fn process_pipeline(&self, v: Slot) -> Result<Rc<RefCell<Pipeline>>, Error> {
        match self.process_handle(v)? {
            ProcessHandle::Pipeline(state) => Ok(state.clone()),
            _ => Err(Error("expected PipelineState".into())),
        }
    }
    fn process_status(&mut self, status: std::process::ExitStatus) -> Result<Slot, Error> {
        let (state, code) = if let Some(signal) = status.signal() {
            (
                2,
                match signal {
                    libc::SIGTERM => 1,
                    libc::SIGKILL => 2,
                    libc::SIGINT => 3,
                    _ => 0,
                },
            )
        } else {
            (1, i64::from(status.code().unwrap_or(0)))
        };
        let state_key = self.allocate_text("state".into())?;
        let code_key = self.allocate_text("code".into())?;
        self.map_new(
            [
                (state_key, Slot::small(state)),
                (code_key, Slot::small(code)),
            ]
            .into_iter(),
        )
    }
    fn process_command(&mut self, n: Native, a: &[Slot]) -> Result<Command, ProviderError> {
        let checked = |s: &str| {
            if s.contains('\0') {
                Err(fail(n, "invalid_input", "Process text contains NUL"))
            } else {
                Ok(s.to_owned())
            }
        };
        let program = checked(self.text(a[1])?)?;
        let directory = checked(self.text(a[3])?)?;
        if program.is_empty() || directory.is_empty() {
            return Err(fail(
                n,
                "invalid_input",
                "Program and directory must be nonempty",
            ));
        }
        let arguments = self
            .elements(a[2])?
            .iter()
            .map(|v| checked(self.text(*v)?))
            .collect::<Result<Vec<_>, ProviderError>>()?;
        if a[4].kind() != MAP {
            return Err(Error("expected environment map".into()).into());
        }
        let mut environment = BTreeMap::new();
        for (k, v) in self.map_entries(a[4])? {
            let name = checked(self.text(k)?)?;
            if name.is_empty() || name.contains('=') {
                return Err(fail(n, "invalid_input", "Invalid process environment name"));
            }
            environment.insert(name, checked(self.text(v)?)?);
        }
        Ok(Command {
            program,
            arguments,
            directory: self.working_directory()?.join(directory),
            environment,
        })
    }
    pub(crate) fn native_process(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        match self.process_execute(n, a) {
            Ok(v) => Ok(Outcome::Value(v)),
            Err(ProviderError::Engine(e)) => Err(e),
            Err(ProviderError::Native(e)) => {
                if matches!(
                    n,
                    Native::ProcessProcessStateExit
                        | Native::ProcessProcessStateCapture
                        | Native::ProcessPipelineStateCapture
                ) {
                    return self
                        .native_io(async move { Err::<bool, _>(e) })
                        .map(Outcome::Value);
                }
                let p = self.native_failure_new(e)?;
                self.failure_new(crate::host::FailureKind::Native(p))
                    .map(Outcome::Failure)
            }
        }
    }
    fn process_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        use Native::*;
        let yes = Slot::boolean(true);
        match n {
            ProcessProcessCurrentId => return Ok(Slot::small(i64::from(std::process::id()))),
            ProcessProcessArguments => {
                let args = self.arguments.clone();
                let values = args
                    .into_iter()
                    .map(|a| self.native_byte_array(a))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(self.allocate_array(values)?);
            }
            ProcessProcessStateCreate => {
                return Ok(self.process_new(ProcessHandle::State(Rc::new(RefCell::new(
                    State::default(),
                ))))?);
            }
            ProcessPipelineStateCreate => {
                return Ok(
                    self.process_new(ProcessHandle::Pipeline(Rc::new(RefCell::new(
                        Pipeline::default(),
                    ))))?,
                );
            }
            _ => {}
        }
        if n.symbol().contains("::captured_") {
            let ProcessHandle::Captured(result) = self.process_handle(a[0])? else {
                return Err(Error("expected captured process".into()).into());
            };
            let result = result.clone();
            return match n {
                ProcessCapturedProcessStatus => Ok(self.process_status(result.statuses[0])?),
                ProcessCapturedPipelineStatuses => {
                    let values = result
                        .statuses
                        .iter()
                        .map(|s| self.process_status(*s))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(self.allocate_array(values)?)
                }
                ProcessCapturedProcessOverflow | ProcessCapturedPipelineOverflowStream => {
                    Ok(Slot::small(result.overflow.0))
                }
                ProcessCapturedPipelineOverflowIndex => Ok(Slot::small(result.overflow.1)),
                ProcessCapturedProcessOutput | ProcessCapturedPipelineOutput => {
                    Ok(self.native_byte_array(result.output.clone())?)
                }
                ProcessCapturedProcessErrors => {
                    Ok(self.native_byte_array(result.errors[0].clone())?)
                }
                ProcessCapturedPipelineErrors => {
                    let values = result
                        .errors
                        .iter()
                        .map(|b| self.native_byte_array(b.clone()))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(self.allocate_array(values)?)
                }
                _ => Err(Error("unsupported captured process native".into()).into()),
            };
        }
        if matches!(n, ProcessPipelineStateAdd | ProcessPipelineStateCapture) {
            let shared = self.process_pipeline(a[0])?;
            let mut state = shared.borrow_mut();
            if state.closed {
                return Err(fail(n, "already_closed", "Pipeline is closed"));
            }
            if state.started || state.capturing {
                return Err(fail(n, "invalid_input", "Pipeline has already started"));
            }
            if n == ProcessPipelineStateAdd {
                state.commands.push(self.process_command(n, a)?);
                return Ok(yes);
            }
            let limit = crate::host::native_file::big(self.integer(a[2])?);
            if limit <= 0.into() {
                return Err(fail(n, "invalid_input", "Capture limit must be positive"));
            }
            use num_traits::ToPrimitive;
            let limit = limit.to_i64().unwrap_or(i64::MAX) as u64;
            if state.commands.is_empty() {
                return Err(fail(n, "invalid_input", "Pipeline must contain a command"));
            }
            let mut spawned = process_host::launch(&state.commands)
                .map_err(|e| ProviderError::Native(host(n, e)))?;
            let children = spawned.iter().map(|s| s.child.clone()).collect::<Vec<_>>();
            state.children = children.clone();
            state.started = true;
            state.capturing = true;
            let input = spawned.first_mut().and_then(|s| s.input.take());
            let output = spawned.last_mut().and_then(|s| s.output.take());
            let errors = spawned.iter_mut().map(|s| s.errors.take()).collect();
            let payload = self.native_bytes(a[1])?;
            drop(state);
            let cancelled = shared.clone();
            let cleanup_children = children.clone();
            let body = async move {
                let result =
                    process_host::capture(children.clone(), input, output, errors, payload, limit)
                        .await;
                if result.is_err() {
                    for child in &children {
                        let _ = child.start_kill();
                    }
                    for child in &children {
                        let _ = child.wait().await;
                    }
                }
                shared.borrow_mut().closed = true;
                result.map_err(|e| host(n, e))
            };
            let cleanup = Box::new(move || {
                cancelled.borrow_mut().closed = true;
                for child in &cleanup_children {
                    let _ = child.start_kill();
                }
                Box::pin(async move {
                    for child in cleanup_children {
                        child.terminate().await.map_err(|e| host(n, e))?;
                    }
                    Ok(())
                })
                    as std::pin::Pin<
                        Box<dyn std::future::Future<Output = Result<(), NativeFailure>>>,
                    >
            });
            return Ok(self.native_io_with_cleanup(body, Some(cleanup))?);
        }
        let shared = self.process_state(a[0])?;
        let mut state = shared.borrow_mut();
        if n == ProcessProcessStateClose {
            state
                .close()
                .map_err(|e| ProviderError::Native(host(n, e)))?;
            return Ok(yes);
        }
        if n == ProcessProcessStateInputClosed {
            return Ok(Slot::boolean(state.closed || state.input_closed));
        }
        if n == ProcessProcessStateInputClose && (state.closed || state.input_closed) {
            return Ok(yes);
        }
        if state.closed {
            return Err(fail(n, "already_closed", "Process is closed"));
        }
        if n == ProcessProcessStateStarted {
            return Ok(Slot::boolean(state.child.is_some()));
        }
        if n == ProcessProcessStateStart {
            if state.child.is_some() {
                return Ok(yes);
            }
            let command = self.process_command(n, a)?;
            let mut spawned = process_host::launch(&[command])
                .map_err(|e| ProviderError::Native(host(n, e)))?
                .remove(0);
            state.child = Some(spawned.child);
            state.input = spawned.input.take();
            state.output = spawned.output.take();
            state.errors = spawned.errors.take();
            return Ok(yes);
        }
        let child = state
            .child
            .clone()
            .ok_or_else(|| fail(n, "invalid_input", "Process has not started"))?;
        match n {
            ProcessProcessStateId => Ok(Slot::small(i64::from(child.pid))),
            ProcessProcessStateRunning => Ok(Slot::boolean(
                child
                    .running()
                    .map_err(|e| ProviderError::Native(host(n, e)))?,
            )),
            ProcessProcessStateExit => Ok(self.native_io(async move {
                child.wait().await.map(Status).map_err(|e| host(n, e))
            })?),
            ProcessProcessStateSignal => {
                let signal = match self.integer(a[1])?.to_i64() {
                    Some(0) => rustix::process::Signal::INT,
                    Some(1) => rustix::process::Signal::TERM,
                    Some(2) => rustix::process::Signal::KILL,
                    _ => return Err(fail(n, "unsupported", "Unsupported process signal")),
                };
                child
                    .signal(signal)
                    .map_err(|e| ProviderError::Native(host(n, e)))?;
                Ok(yes)
            }
            ProcessProcessStateInputClose => {
                state.input_closed = true;
                state.input.take();
                Ok(yes)
            }
            ProcessProcessStateInputFlush | ProcessProcessStateInputWrite => {
                if state.input_closed {
                    return Err(fail(n, "already_closed", "Process input is closed"));
                }
                if n == ProcessProcessStateInputFlush {
                    return Ok(yes);
                }
                let bytes = self.native_bytes(a[1])?;
                if bytes.is_empty() {
                    return Ok(Slot::small(0));
                }
                let input = state
                    .input
                    .as_mut()
                    .ok_or_else(|| fail(n, "broken_pipe", "Process input unavailable"))?;
                input
                    .write_all(&bytes)
                    .and_then(|()| input.flush())
                    .map_err(|e| ProviderError::Native(host(n, e)))?;
                Ok(Slot::small(bytes.len() as i64))
            }
            ProcessProcessStateOutputRead | ProcessProcessStateErrorsRead => {
                let limit = crate::host::native_file::big(self.integer(a[1])?);
                if limit <= 0.into() {
                    return Err(fail(n, "invalid_input", "Read limit must be positive"));
                }
                use num_traits::ToPrimitive;
                let limit = limit.to_i64().unwrap_or(i64::MAX).min(65536) as usize;
                let pipe = if n == ProcessProcessStateOutputRead {
                    &mut state.output
                } else {
                    &mut state.errors
                };
                let Some(pipe) = pipe else {
                    return Ok(self.native_byte_array(vec![])?);
                };
                let mut bytes = vec![0; limit];
                let size = loop {
                    match pipe.read(&mut bytes) {
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        result => break result.map_err(|e| ProviderError::Native(host(n, e)))?,
                    }
                };
                bytes.truncate(size);
                Ok(self.native_byte_array(bytes)?)
            }
            ProcessProcessStateCapture => {
                let limit = crate::host::native_file::big(self.integer(a[2])?);
                if limit <= 0.into() {
                    return Err(fail(n, "invalid_input", "Capture limit must be positive"));
                }
                use num_traits::ToPrimitive;
                let limit = limit.to_i64().unwrap_or(i64::MAX) as u64;
                if state.capturing || state.input_closed {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Process input unavailable for capture",
                    ));
                }
                let payload = self.native_bytes(a[1])?;
                state.capturing = true;
                state.input_closed = true;
                let input = state.input.take();
                let output = state.output.take();
                let errors = vec![state.errors.take()];
                drop(state);
                let cancelled = shared.clone();
                let cleanup_child = child.clone();
                let body = async move {
                    let result = process_host::capture(
                        vec![child.clone()],
                        input,
                        output,
                        errors,
                        payload,
                        limit,
                    )
                    .await;
                    if result.is_err() {
                        let _ = child.terminate().await;
                    }
                    shared.borrow_mut().cancel();
                    result.map_err(|e| host(n, e))
                };
                let cleanup = Box::new(move || {
                    cancelled.borrow_mut().cancel();
                    let _ = cleanup_child.start_kill();
                    Box::pin(async move { cleanup_child.terminate().await.map_err(|e| host(n, e)) })
                        as std::pin::Pin<
                            Box<dyn std::future::Future<Output = Result<(), NativeFailure>>>,
                        >
                });
                Ok(self.native_io_with_cleanup(body, Some(cleanup))?)
            }
            _ => Err(Error("unsupported process native".into()).into()),
        }
    }
}
