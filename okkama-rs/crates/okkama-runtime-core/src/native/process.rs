//! `runtime/process` — перенос `ProcessNatives` и `PosixSpawn` JVM: потомки
//! с точным статусом `waitpid` (выход с кодом или гибель от сигнала),
//! сигналы `kill(2)`, захват выводов с пределом и конвейеры с каналами ОС.
//! Поток-жнец на каждого потомка собирает статус сразу после запуска.
#![allow(unsafe_code)]

use super::support::{array, byte_array, bytes, entries, handle, int, int_map, int_value, new_handle, string, strings};
use super::Error;
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{IntValue, Value, abi::Native};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

const DOMAIN: &str = "core.process";
const CHUNK: usize = 64 * 1024;

fn raise(code: &str, operation: &str, message: &str) -> Rc<Failure> {
    Failure::new(Kind::Native(Rc::new(crate::abi::NativeFailure {
        domain: DOMAIN.into(),
        code: code.into(),
        operation: operation.into(),
        message: message.into(),
    })))
}

fn as_error(failure: Rc<Failure>) -> Error {
    match &failure.kind {
        Kind::Native(native) => Error::Failure((**native).clone()),
        _ => Error::Trap(failure.message().to_string()),
    }
}

/// Код отказа запуска по errno, как `Host.code` над текстом `PosixSpawn`.
fn spawn_code(error: &std::io::Error) -> &'static str {
    match error.raw_os_error() {
        Some(libc::ENOENT) => "not_found",
        Some(libc::EACCES) => "permission_denied",
        Some(libc::ENOTDIR) => "invalid_input",
        Some(_) => "unexpected",
        None => crate::host::code(error),
    }
}

fn broken_pipe(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::BrokenPipe || error.raw_os_error() == Some(libc::EPIPE)
}

// ---- потомок ----

/// Статус потомка: сырое значение `waitpid` после завершения.
#[derive(Debug, Default)]
struct Exit {
    done: Mutex<Option<std::process::ExitStatus>>,
    changed: Condvar,
}

#[derive(Debug)]
struct Child {
    pid: i32,
    exit: Arc<Exit>,
}

impl Child {
    fn alive(&self) -> bool {
        self.exit.done.lock().map(|done| done.is_none()).unwrap_or(false)
    }

    fn signal(&self, number: i32) -> bool {
        if !self.alive() {
            return false;
        }
        // SAFETY: kill(2) потомку, ещё не собранному жнецом.
        unsafe { libc::kill(self.pid, number) == 0 }
    }

    fn wait(&self) -> std::process::ExitStatus {
        let mut done = self.exit.done.lock().expect("exit lock");
        loop {
            if let Some(status) = *done {
                return status;
            }
            done = self.exit.changed.wait(done).expect("exit lock");
        }
    }

    /// Статус как карта `{code, state}`: 1 — выход, 2 — сигнал.
    fn project(&self) -> Value {
        let status = self.wait();
        match status.signal() {
            Some(signal) => {
                let portable = match signal {
                    15 => 1,
                    9 => 2,
                    2 => 3,
                    _ => 0,
                };
                int_map([("code", portable), ("state", 2)])
            }
            None => int_map([("code", i64::from(status.code().unwrap_or(0))), ("state", 1)]),
        }
    }

    fn kill(&self) {
        self.signal(9);
        self.wait();
    }
}

struct Spawned {
    child: Arc<Child>,
    input: Option<std::process::ChildStdin>,
    output: Option<std::process::ChildStdout>,
    errors: Option<std::process::ChildStderr>,
}

fn reap(mut process: std::process::Child) -> Arc<Child> {
    let exit = Arc::new(Exit::default());
    let child = Arc::new(Child {
        pid: process.id() as i32,
        exit: exit.clone(),
    });
    std::thread::Builder::new()
        .name(format!("okkama-reap-{}", child.pid))
        .spawn(move || {
            let status = process.wait().unwrap_or_else(|_| std::process::ExitStatus::from_raw(0));
            if let Ok(mut done) = exit.done.lock() {
                *done = Some(status);
            }
            exit.changed.notify_all();
        })
        .expect("reaper thread");
    child
}

#[derive(Debug, Clone)]
struct Command {
    program: String,
    arguments: Vec<String>,
    directory: String,
    environment: Vec<(String, String)>,
}

fn checked(value: &str, description: &str, operation: &str) -> Result<String, Rc<Failure>> {
    if value.contains('\0') {
        return Err(raise("invalid_input", operation, &format!("{description} contains a zero byte")));
    }
    Ok(value.to_string())
}

fn command(kind: &str, args: &[Value], operation: &str) -> Result<Command, Error> {
    let program = string(&args[1])?;
    let invalid = |message: String| as_error(raise("invalid_input", operation, &message));
    let program = checked(program, &format!("{kind} program"), operation).map_err(as_error)?;
    if program.is_empty() {
        return Err(invalid(format!("{kind} program is empty")));
    }
    let arguments = strings(&args[2])?
        .into_iter()
        .map(|argument| checked(&argument, &format!("{kind} argument"), operation))
        .collect::<Result<Vec<_>, _>>()
        .map_err(as_error)?;
    let directory = checked(string(&args[3])?, &format!("{kind} directory"), operation).map_err(as_error)?;
    if directory.is_empty() {
        return Err(invalid(format!("{kind} directory is empty")));
    }
    let process = kind == "Process";
    let mut environment = Vec::new();
    for (name, value) in entries(&args[4])? {
        let name = checked(&name, &format!("{kind} environment name"), operation).map_err(as_error)?;
        if name.is_empty() {
            return Err(invalid(
                if process { "Process environment name is empty" } else { "Invalid pipeline environment name" }.into(),
            ));
        }
        if name.contains('=') {
            return Err(invalid(
                if process { "Process environment name contains equals" } else { "Invalid pipeline environment name" }.into(),
            ));
        }
        let value = checked(string(&value)?, &format!("{kind} environment value"), operation).map_err(as_error)?;
        environment.push((name, value));
    }
    Ok(Command {
        program,
        arguments,
        directory,
        environment,
    })
}

fn executable(path: &std::path::Path) -> bool {
    let Ok(text) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: строка с завершающим нулём.
    path.is_file() && unsafe { libc::access(text.as_ptr(), libc::X_OK) } == 0
}

/// Программа без `/` ищется по PATH окружения потомка (`execvp` Perl); не
/// найденная — относительно каталога потомка, как `posix_spawn` после chdir.
fn resolve(command: &Command, directory: &std::path::Path) -> String {
    if command.program.contains('/') {
        return command.program.clone();
    }
    let path = command
        .environment
        .iter()
        .find(|(name, _)| name == "PATH")
        .map_or("/usr/bin:/bin", |(_, value)| value.as_str());
    for entry in path.split(':') {
        let base = if entry.is_empty() {
            directory.to_path_buf()
        } else if std::path::Path::new(entry).is_absolute() {
            std::path::PathBuf::from(entry)
        } else {
            directory.join(entry)
        };
        let candidate = base.join(&command.program);
        if executable(&candidate) {
            return candidate.to_string_lossy().into_owned();
        }
    }
    directory.join(&command.program).to_string_lossy().into_owned()
}

/// Конвейер: stdin первого, stdout последнего и stderr каждого — каналы родителя.
fn launch(commands: &[Command]) -> std::io::Result<Vec<Spawned>> {
    use std::process::Stdio;
    let mut spawned: Vec<Spawned> = Vec::new();
    let mut previous: Option<std::process::ChildStdout> = None;
    for (index, command) in commands.iter().enumerate() {
        let directory = crate::host::path(&command.directory);
        let program = resolve(command, &directory);
        let mut builder = std::process::Command::new(&program);
        builder
            .args(&command.arguments)
            .current_dir(&directory)
            .env_clear()
            .envs(command.environment.iter().map(|(name, value)| (name, value)))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match previous.take() {
            Some(output) => builder.stdin(Stdio::from(output)),
            None => builder.stdin(Stdio::piped()),
        };
        let mut process = match builder.spawn() {
            Ok(process) => process,
            Err(error) => {
                for done in &spawned {
                    done.child.kill();
                }
                return Err(error);
            }
        };
        let input = process.stdin.take();
        let errors = process.stderr.take();
        let output = if index == commands.len() - 1 {
            process.stdout.take()
        } else {
            previous = process.stdout.take();
            None
        };
        spawned.push(Spawned {
            child: reap(process),
            input,
            output,
            errors,
        });
    }
    Ok(spawned)
}

// ---- состояние процесса ----

#[derive(Default)]
struct ProcessState {
    child: Option<Arc<Child>>,
    input: Option<std::process::ChildStdin>,
    output: Option<std::process::ChildStdout>,
    errors: Option<std::process::ChildStderr>,
    input_closed: bool,
    closed: bool,
    capturing: bool,
}

type ProcessRef = Rc<RefCell<ProcessState>>;

fn state(value: &Value, operation: &str) -> Result<ProcessRef, Rc<Failure>> {
    let state = handle::<ProcessRef>(value).map_err(|_| raise("invalid_input", operation, "Invalid process"))?.clone();
    if state.borrow().closed {
        return Err(raise("already_closed", operation, "Process is already closed"));
    }
    Ok(state)
}

fn started(value: &Value, operation: &str) -> Result<ProcessRef, Rc<Failure>> {
    let state = state(value, operation)?;
    if state.borrow().child.is_none() {
        return Err(raise("invalid_input", operation, "Process has not been started"));
    }
    Ok(state)
}

fn close(state: &mut ProcessState) {
    if state.closed {
        return;
    }
    state.closed = true;
    if let Some(child) = &state.child {
        child.kill();
        state.input = None;
        state.output = None;
        state.errors = None;
    }
}

fn child(state: &ProcessRef) -> Arc<Child> {
    state.borrow().child.clone().expect("started process")
}

fn read_stream(stream: Option<&mut dyn Read>, limit: &IntValue, operation: &str) -> Result<Value, Error> {
    if limit.signum() <= 0 {
        return Err(as_error(raise("invalid_input", operation, "Process read size must be positive")));
    }
    let size = limit.to_i64().map_or(CHUNK, |limit| (limit as usize).min(CHUNK));
    let Some(stream) = stream else {
        return Ok(byte_array([]));
    };
    let mut buffer = vec![0u8; size];
    loop {
        match stream.read(&mut buffer) {
            Ok(count) => return Ok(byte_array(buffer[..count].iter().copied())),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(as_error(raise(crate::host::code(&error), operation, "Cannot read process output")));
            }
        }
    }
}

// ---- захват ----

#[derive(Default)]
struct Shared {
    overflow: Mutex<(i64, i64)>,
    problem: Mutex<Option<std::io::Error>>,
}

/// Общий захват: ввод в первый процесс, выводы, переполнение, статусы.
struct Capture {
    children: Vec<Arc<Child>>,
    input: Option<std::process::ChildStdin>,
    output: Option<std::process::ChildStdout>,
    errors: Vec<Option<std::process::ChildStderr>>,
    payload: Vec<u8>,
    limit: u64,
}

#[derive(Debug)]
struct Captured {
    output: Vec<u8>,
    errors: Vec<Vec<u8>>,
    overflow: (i64, i64),
}

fn kill_all(children: &[Arc<Child>]) {
    for child in children {
        child.signal(9);
    }
}

impl Capture {
    fn run(mut self) -> std::io::Result<Captured> {
        let shared = Arc::new(Shared {
            overflow: Mutex::new((0, -1)),
            problem: Mutex::new(None),
        });
        let children = Arc::new(self.children.clone());
        // Ввод закрывает переполнение: держим его общим.
        let input = Arc::new(Mutex::new(self.input.take()));
        let limit = self.limit;
        let drain = |stream: Box<dyn Read + Send>, kind: i64, index: i64| {
            let shared = shared.clone();
            let children = children.clone();
            let input = input.clone();
            std::thread::Builder::new()
                .name("okkama-capture".into())
                .spawn(move || {
                    let mut stream = stream;
                    let mut target = Vec::new();
                    let mut buffer = vec![0u8; CHUNK];
                    loop {
                        match stream.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(count) => {
                                target.extend_from_slice(&buffer[..count]);
                                if target.len() as u64 > limit {
                                    let mut overflow = shared.overflow.lock().expect("capture lock");
                                    if overflow.0 == 0 {
                                        *overflow = (kind, index);
                                        if let Ok(mut input) = input.lock() {
                                            *input = None;
                                        }
                                        kill_all(&children);
                                    }
                                }
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(error) => {
                                let mut problem = shared.problem.lock().expect("capture lock");
                                if problem.is_none() {
                                    *problem = Some(error);
                                }
                                if let Ok(mut input) = input.lock() {
                                    *input = None;
                                }
                                kill_all(&children);
                                break;
                            }
                        }
                    }
                    target
                })
                .expect("capture thread")
        };
        let last = self.children.len() as i64 - 1;
        let output = self.output.take().map(|stream| drain(Box::new(stream), 1, last));
        let errors: Vec<_> = self
            .errors
            .drain(..)
            .enumerate()
            .map(|(index, stream)| stream.map(|stream| drain(Box::new(stream), 2, index as i64)))
            .collect();
        let mut offset = 0;
        while offset < self.payload.len() && shared.overflow.lock().expect("capture lock").0 == 0 {
            let size = CHUNK.min(self.payload.len() - offset);
            let written = {
                let mut guard = input.lock().expect("capture lock");
                match guard.as_mut() {
                    Some(stream) => stream.write_all(&self.payload[offset..offset + size]).and_then(|()| stream.flush()),
                    None => Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
                }
            };
            if let Err(error) = written {
                if !broken_pipe(&error) && shared.overflow.lock().expect("capture lock").0 == 0 {
                    let mut problem = shared.problem.lock().expect("capture lock");
                    if problem.is_none() {
                        *problem = Some(error);
                    }
                    kill_all(&self.children);
                }
                break;
            }
            offset += size;
        }
        if let Ok(mut input) = input.lock() {
            *input = None;
        }
        let output = output.map(|thread| thread.join().unwrap_or_default()).unwrap_or_default();
        let errors: Vec<Vec<u8>> = errors
            .into_iter()
            .map(|thread| thread.map(|thread| thread.join().unwrap_or_default()).unwrap_or_default())
            .collect();
        let problem = shared.problem.lock().expect("capture lock").take();
        for child in &self.children {
            if problem.is_some() {
                child.signal(9);
            }
            child.wait();
        }
        if let Some(problem) = problem {
            return Err(problem);
        }
        let overflow = *shared.overflow.lock().expect("capture lock");
        Ok(Captured { output, errors, overflow })
    }
}

#[derive(Debug)]
struct CapturedProcess {
    status: Value,
    captured: Captured,
}

#[derive(Debug)]
struct CapturedPipeline {
    statuses: Vec<Value>,
    captured: Captured,
}

fn capture(
    capture: Capture,
    operation: String,
    message: &'static str,
    finish: impl FnOnce() + 'static,
    result: impl FnOnce(Captured) -> Value + 'static,
) -> Value {
    let promise = sched::promise();
    let target = promise.clone();
    let op = sched::offload(
        move || capture.run(),
        move |outcome: std::io::Result<Captured>| {
            finish();
            match outcome {
                Ok(captured) => sched::settle(&target, Ok(result(captured))),
                Err(error) => sched::settle(&target, Err(raise(crate::host::code(&error), &operation, message))),
            }
        },
    );
    promise.listen(move || op.detach());
    Value::Future(promise)
}

fn resolved(failure: Rc<Failure>) -> Value {
    Value::Future(sched::resolved(Err(failure)))
}

// ---- конвейер ----

#[derive(Default)]
struct PipelineState {
    commands: Vec<Command>,
    children: Vec<Arc<Child>>,
    started: bool,
    closed: bool,
    capturing: bool,
}

type PipelineRef = Rc<RefCell<PipelineState>>;

fn close_pipeline(state: &mut PipelineState) {
    if state.closed {
        return;
    }
    state.closed = true;
    for child in &state.children {
        child.kill();
    }
}

fn limit_of(limit: &IntValue) -> u64 {
    limit.to_i64().map_or(i64::MAX as u64, |limit| limit as u64)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.name();
    let operation = native.symbol();
    match name {
        "process_current_id" => Ok(int_value(i64::from(std::process::id()))),
        "process_state_create" => Ok(new_handle(Rc::new(RefCell::new(ProcessState::default())))),
        "process_state_start" => {
            let state = state(&args[0], operation).map_err(as_error)?;
            if state.borrow().child.is_some() {
                return Ok(Value::Bool(true));
            }
            let command = command("Process", args, operation)?;
            let mut spawned = launch(&[command])
                .map_err(|error| as_error(raise(spawn_code(&error), operation, "Cannot configure or execute process")))?;
            let spawned = spawned.remove(0);
            let mut borrowed = state.borrow_mut();
            borrowed.child = Some(spawned.child);
            borrowed.input = spawned.input;
            borrowed.output = spawned.output;
            borrowed.errors = spawned.errors;
            Ok(Value::Bool(true))
        }
        "process_state_started" => Ok(Value::Bool(state(&args[0], operation).map_err(as_error)?.borrow().child.is_some())),
        "process_state_id" => {
            let state = started(&args[0], operation).map_err(as_error)?;
            Ok(int_value(i64::from(child(&state).pid)))
        }
        "process_state_running" => {
            let state = started(&args[0], operation).map_err(as_error)?;
            Ok(Value::Bool(child(&state).alive()))
        }
        "process_state_exit" => {
            let state = match started(&args[0], operation) {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(failure)),
            };
            let process = child(&state);
            if !process.alive() {
                return Ok(Value::Future(sched::resolved(Ok(process.project()))));
            }
            let promise = sched::promise();
            let target = promise.clone();
            let waiting = process.clone();
            let op = sched::offload(
                move || {
                    waiting.wait();
                },
                move |()| sched::settle(&target, Ok(process.project())),
            );
            promise.listen(move || op.detach());
            Ok(Value::Future(promise))
        }
        "process_state_signal" => {
            let state = started(&args[0], operation).map_err(as_error)?;
            let process = child(&state);
            if !process.alive() {
                return Err(as_error(raise("invalid_input", operation, "Process has already completed")));
            }
            match int(&args[1])?.to_i64() {
                Some(code @ 0..=2) => {
                    let number = [libc::SIGINT, libc::SIGTERM, libc::SIGKILL][code as usize];
                    if !process.signal(number) {
                        return Err(as_error(raise("unexpected", operation, "Cannot signal child process")));
                    }
                    Ok(Value::Bool(true))
                }
                _ => Err(as_error(raise("unsupported", operation, "Process signal is unsupported"))),
            }
        }
        "process_state_close" => {
            close(&mut handle::<ProcessRef>(&args[0])?.borrow_mut());
            Ok(Value::Bool(true))
        }
        "process_state_input_write" => {
            let state = started(&args[0], operation).map_err(as_error)?;
            if state.borrow().input_closed {
                return Err(as_error(raise("already_closed", operation, "Process input is already closed")));
            }
            let payload = bytes(&args[1])?;
            if payload.is_empty() {
                return Ok(int_value(0));
            }
            let mut borrowed = state.borrow_mut();
            let written = match borrowed.input.as_mut() {
                Some(input) => input.write_all(&payload).and_then(|()| input.flush()),
                None => Err(std::io::Error::other("Stream closed")),
            };
            written.map_err(|error| {
                let code = if broken_pipe(&error) || error.to_string().contains("Stream closed") {
                    "broken_pipe"
                } else {
                    crate::host::code(&error)
                };
                as_error(raise(code, operation, "Cannot write process input"))
            })?;
            Ok(int_value(payload.len() as i64))
        }
        "process_state_input_flush" => {
            let state = started(&args[0], operation).map_err(as_error)?;
            if state.borrow().input_closed {
                return Err(as_error(raise("already_closed", operation, "Process input is already closed")));
            }
            Ok(Value::Bool(true))
        }
        "process_state_input_closed" => {
            let state = handle::<ProcessRef>(&args[0])?.borrow();
            Ok(Value::Bool(state.closed || state.input_closed))
        }
        "process_state_input_close" => {
            let state = handle::<ProcessRef>(&args[0])?.clone();
            let mut borrowed = state.borrow_mut();
            if borrowed.closed || borrowed.input_closed {
                return Ok(Value::Bool(true));
            }
            if borrowed.child.is_none() {
                return Err(as_error(raise("invalid_input", operation, "Process has not been started")));
            }
            borrowed.input_closed = true;
            borrowed.input = None;
            Ok(Value::Bool(true))
        }
        "process_state_output_read" | "process_state_errors_read" => {
            let state = started(&args[0], operation).map_err(as_error)?;
            let limit = int(&args[1])?;
            let mut borrowed = state.borrow_mut();
            let stream: Option<&mut dyn Read> = if name == "process_state_output_read" {
                borrowed.output.as_mut().map(|stream| stream as &mut dyn Read)
            } else {
                borrowed.errors.as_mut().map(|stream| stream as &mut dyn Read)
            };
            read_stream(stream, limit, operation)
        }
        "process_state_capture" => {
            let limit = int(&args[2])?;
            let checked = || -> Result<ProcessRef, Rc<Failure>> {
                if limit.signum() <= 0 {
                    return Err(raise("invalid_input", operation, "Capture limit must be positive"));
                }
                let known = handle::<ProcessRef>(&args[0]).map_err(|_| raise("invalid_input", operation, "Invalid process"))?;
                if known.borrow().capturing {
                    return Err(raise("invalid_input", operation, "Process capture is already active"));
                }
                let state = started(&args[0], operation)?;
                if state.borrow().input_closed {
                    return Err(raise("invalid_input", operation, "Process input is unavailable"));
                }
                Ok(state)
            };
            let state = match checked() {
                Ok(state) => state,
                Err(failure) => return Ok(resolved(failure)),
            };
            let payload = bytes(&args[1])?;
            let job = {
                let mut borrowed = state.borrow_mut();
                borrowed.capturing = true;
                borrowed.input_closed = true;
                Capture {
                    children: vec![borrowed.child.clone().expect("started")],
                    input: borrowed.input.take(),
                    output: borrowed.output.take(),
                    errors: vec![borrowed.errors.take()],
                    payload,
                    limit: limit_of(limit),
                }
            };
            let process = child(&state);
            let finished = state.clone();
            Ok(capture(
                job,
                operation.to_string(),
                "Cannot capture process output",
                move || close(&mut finished.borrow_mut()),
                move |mut captured| {
                    let errors = captured.errors.pop().unwrap_or_default();
                    captured.errors = vec![errors];
                    new_handle(CapturedProcess {
                        status: process.project(),
                        captured,
                    })
                },
            ))
        }
        "captured_process_status" => Ok(handle::<CapturedProcess>(&args[0])?.status.clone()),
        "captured_process_overflow" => Ok(int_value(handle::<CapturedProcess>(&args[0])?.captured.overflow.0)),
        "captured_process_output" => Ok(byte_array(handle::<CapturedProcess>(&args[0])?.captured.output.clone())),
        "captured_process_errors" => Ok(byte_array(handle::<CapturedProcess>(&args[0])?.captured.errors[0].clone())),
        "pipeline_state_create" => Ok(new_handle(Rc::new(RefCell::new(PipelineState::default())))),
        "pipeline_state_add" => {
            let state = handle::<PipelineRef>(&args[0])?.clone();
            if state.borrow().closed {
                return Err(as_error(raise("already_closed", operation, "Pipeline is already closed")));
            }
            if state.borrow().started {
                return Err(as_error(raise("invalid_input", operation, "Pipeline has already started")));
            }
            let command = command("Pipeline", args, operation)?;
            state.borrow_mut().commands.push(command);
            Ok(Value::Bool(true))
        }
        "pipeline_state_capture" => {
            let limit = int(&args[2])?;
            let state = handle::<PipelineRef>(&args[0])?.clone();
            let launched = || -> Result<Vec<Spawned>, Rc<Failure>> {
                if limit.signum() <= 0 {
                    return Err(raise("invalid_input", operation, "Capture limit must be positive"));
                }
                let borrowed = state.borrow();
                if borrowed.capturing {
                    return Err(raise("invalid_input", operation, "Pipeline capture is already active"));
                }
                if borrowed.closed {
                    return Err(raise("already_closed", operation, "Pipeline is already closed"));
                }
                if borrowed.commands.is_empty() {
                    return Err(raise("invalid_input", operation, "Pipeline must contain a command"));
                }
                launch(&borrowed.commands)
                    .map_err(|error| raise(spawn_code(&error), operation, "Cannot configure or execute pipeline command"))
            };
            let mut spawned = match launched() {
                Ok(spawned) => spawned,
                Err(failure) => return Ok(resolved(failure)),
            };
            let payload = bytes(&args[1])?;
            let children: Vec<Arc<Child>> = spawned.iter().map(|spawned| spawned.child.clone()).collect();
            {
                let mut borrowed = state.borrow_mut();
                borrowed.started = true;
                borrowed.capturing = true;
                borrowed.children = children.clone();
            }
            let input = spawned.first_mut().and_then(|first| first.input.take());
            let output = spawned.last_mut().and_then(|last| last.output.take());
            let errors = spawned.iter_mut().map(|spawned| spawned.errors.take()).collect();
            let job = Capture {
                children: children.clone(),
                input,
                output,
                errors,
                payload,
                limit: limit_of(limit),
            };
            let finished = state.clone();
            Ok(capture(
                job,
                operation.to_string(),
                "Cannot capture pipeline output",
                move || close_pipeline(&mut finished.borrow_mut()),
                move |captured| {
                    new_handle(CapturedPipeline {
                        statuses: children.iter().map(|child| child.project()).collect(),
                        captured,
                    })
                },
            ))
        }
        "captured_pipeline_statuses" => Ok(array(handle::<CapturedPipeline>(&args[0])?.statuses.clone())),
        "captured_pipeline_overflow_stream" => Ok(int_value(handle::<CapturedPipeline>(&args[0])?.captured.overflow.0)),
        "captured_pipeline_overflow_index" => Ok(int_value(handle::<CapturedPipeline>(&args[0])?.captured.overflow.1)),
        "captured_pipeline_output" => Ok(byte_array(handle::<CapturedPipeline>(&args[0])?.captured.output.clone())),
        "captured_pipeline_errors" => Ok(array(
            handle::<CapturedPipeline>(&args[0])?
                .captured
                .errors
                .iter()
                .map(|errors| byte_array(errors.clone()))
                .collect(),
        )),
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}

