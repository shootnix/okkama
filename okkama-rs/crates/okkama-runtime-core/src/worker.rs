//! Процессы-воркеры `spawn` синхронного действия (§18.1) — протокол `Worker`
//! JVM, общий для исполнителей. Родитель запускает ту же программу тем же
//! движком с `OKKAMA_SPAWN_WORKER`; вместо `main` она читает запрос из stdin:
//! каталог, символ функции-входа и аргументы. Ответ — последняя рамка stdout
//! `OKW1`: печать самого действия идёт перед ней.
use crate::failure::{Failure, Kind as FailureKind};
use crate::heap::{ArrayHeap, maps};
use crate::{IntValue, Value, sched};
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Mutex;

pub const MODE: &str = "OKKAMA_SPAWN_WORKER";
const FRAME: &[u8; 4] = b"OKW1";

const NONE: u8 = 0;
const INT: u8 = 1;
const FLOAT: u8 = 2;
const STRING: u8 = 3;
const CHAR: u8 = 4;
const BYTE: u8 = 5;
const BOOL: u8 = 6;
const ARRAY: u8 = 7;
const MAP: u8 = 8;

static COMMAND: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Команда запуска той же программы для воркера (`okkama run …`).
pub fn set_command(command: Vec<String>) {
    if let Ok(mut slot) = COMMAND.lock() {
        *slot = Some(command);
    }
}

/// Процесс — воркер родителя.
pub fn active() -> bool {
    std::env::var_os(MODE).is_some()
}

fn text(out: &mut Vec<u8>, value: &str) {
    out.extend((value.len() as u32).to_be_bytes());
    out.extend(value.as_bytes());
}

fn trap(message: impl Into<String>) -> Rc<Failure> {
    Failure::new(FailureKind::Program(message.into()))
}

/// Имя типа непередаваемого значения в отказе (`typeName` JVM).
pub type TypeName<'a> = &'a dyn Fn(&Value) -> String;

/// Передаваемые значения: скаляры, массивы и maps из них (§18.1).
fn encode(out: &mut Vec<u8>, value: &Value, type_name: TypeName<'_>) -> Result<(), Rc<Failure>> {
    match value {
        Value::Undef => out.push(NONE),
        Value::Int(number) => {
            out.push(INT);
            text(out, &number.to_string());
        }
        Value::Float(number) => {
            out.push(FLOAT);
            out.extend(number.to_bits().to_be_bytes());
        }
        Value::Str(string) => {
            out.push(STRING);
            text(out, string);
        }
        Value::Char(character) => {
            out.push(CHAR);
            out.extend(u32::from(*character).to_be_bytes());
        }
        Value::Byte(byte) => {
            out.push(BYTE);
            out.push(*byte);
        }
        Value::Bool(flag) => {
            out.push(BOOL);
            out.push(u8::from(*flag));
        }
        Value::Array(_) => {
            let items = ArrayHeap.elements(value).map_err(|error| trap(error.message))?;
            out.push(ARRAY);
            out.extend((items.len() as u32).to_be_bytes());
            for item in items {
                encode(out, item, type_name)?;
            }
        }
        Value::Map(_) => {
            let entries = maps::entries(value).map_err(|error| trap(error.message))?;
            out.push(MAP);
            out.extend((entries.len() as u32).to_be_bytes());
            for (key, item) in entries {
                encode(out, &key.to_value(), type_name)?;
                encode(out, item, type_name)?;
            }
        }
        other => return Err(trap(format!("spawn cannot transfer value of type {}", type_name(other)))),
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(count).filter(|end| *end <= self.bytes.len()).ok_or("truncated spawn value")?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("four")))
    }

    fn text(&mut self) -> Result<String, String> {
        let size = self.u32()? as usize;
        String::from_utf8(self.take(size)?.to_vec()).map_err(|_| "invalid spawn text".to_string())
    }

    fn value(&mut self) -> Result<Value, String> {
        let tag = self.take(1)?[0];
        Ok(match tag {
            NONE => Value::Undef,
            INT => Value::Int(IntValue::parse(&self.text()?).ok_or("invalid spawn integer")?),
            FLOAT => Value::Float(f64::from_bits(u64::from_be_bytes(self.take(8)?.try_into().expect("eight")))),
            STRING => Value::Str(self.text()?.into()),
            CHAR => Value::Char(char::from_u32(self.u32()?).ok_or("invalid spawn character")?),
            BYTE => Value::Byte(self.take(1)?[0]),
            BOOL => Value::Bool(self.take(1)?[0] != 0),
            ARRAY => {
                let size = self.u32()? as usize;
                let items = (0..size).map(|_| self.value()).collect::<Result<Vec<_>, _>>()?;
                ArrayHeap.allocate(items)
            }
            MAP => {
                let size = self.u32()? as usize;
                let mut entries = Vec::with_capacity(size);
                for _ in 0..size {
                    let key = self.value()?;
                    entries.push((key, self.value()?));
                }
                maps::from_entries(entries).map_err(|error| error.message)?
            }
            other => return Err(format!("invalid spawn value tag {other}")),
        })
    }
}

/// `worker.bind`: действие, которое `worker.spawn` исполнит в воркере.
pub fn bind(action: &Value, entry: Value, fields: Vec<Value>) -> Result<Value, String> {
    let Value::Fn(closure) = action else {
        return Err("worker.bind expects an action".into());
    };
    Ok(Value::Fn(Rc::new(crate::heap::Closure {
        function: closure.function,
        captures: closure.captures.clone(),
        worker: Some(Box::new(crate::heap::Bound { entry, fields })),
    })))
}

const UNBOUND: &str = "spawn requires a synchronous bound object action whose receiver fields reconstruct it";

/// Вход и аргументы запроса: `direct` — `entry` сам вход; иначе bound action.
/// `symbol` даёт символ функции значения-функции.
pub fn target(entry: &Value, args: Vec<Value>, direct: bool, symbol: &dyn Fn(usize) -> Option<String>) -> Result<(String, Vec<Value>), Rc<Failure>> {
    let function = |value: &Value| match value {
        Value::Fn(closure) => symbol(closure.function),
        _ => None,
    };
    if direct {
        return Ok((function(entry).ok_or_else(|| trap(UNBOUND))?, args));
    }
    match entry {
        Value::Fn(closure) if closure.worker.is_some() => {
            let bound = closure.worker.as_ref().expect("bound");
            let mut all = bound.fields.clone();
            all.extend(args);
            Ok((function(&bound.entry).ok_or_else(|| trap(UNBOUND))?, all))
        }
        _ => Err(trap(UNBOUND)),
    }
}

/// Запустить воркер: Future результата его входа.
pub fn spawn(target: String, args: Vec<Value>, type_name: TypeName<'_>) -> Result<Value, Rc<Failure>> {
    let mut request = Vec::new();
    text(&mut request, &crate::host::cwd().to_string_lossy());
    text(&mut request, &target);
    request.extend((args.len() as u32).to_be_bytes());
    for value in &args {
        encode(&mut request, value, type_name)?;
    }
    let command = COMMAND
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_else(|| std::env::args().collect());
    let environment = crate::host::environment();
    let cwd = crate::host::cwd();
    let promise = sched::promise();
    sched::register(&promise);
    let child: std::sync::Arc<Mutex<Option<u32>>> = std::sync::Arc::new(Mutex::new(None));
    let running = child.clone();
    let target_promise = promise.clone();
    let op = sched::offload(
        move || run(&command, &environment, &cwd, &request, &running),
        move |outcome: Result<Result<Vec<u8>, String>, String>| {
            let result = match outcome {
                Err(error) => Err(trap(format!("cannot execute spawn worker: {error}"))),
                Ok(Err(message)) => Err(trap(message)),
                Ok(Ok(value)) => {
                    let mut reader = Reader { bytes: &value, at: 0 };
                    reader.value().map_err(|error| trap(format!("cannot execute spawn worker: {error}")))
                }
            };
            sched::settle(&target_promise, result);
        },
    );
    promise.listen(move || {
        op.detach();
        if let Ok(pid) = child.lock() {
            if let Some(pid) = *pid {
                // SIGTERM ещё не собранному потомку (как `destroy` JVM).
                crate::host::signal(pid, 15);
            }
        }
    });
    Ok(Value::Future(promise))
}

/// Исполнить запрос в дочернем процессе: значение или текст отказа.
fn run(
    command: &[String],
    environment: &std::collections::BTreeMap<String, String>,
    cwd: &std::path::Path,
    request: &[u8],
    child: &Mutex<Option<u32>>,
) -> Result<Result<Vec<u8>, String>, String> {
    let (program, rest) = command.split_first().ok_or("empty worker command")?;
    let mut process = std::process::Command::new(program)
        .args(rest)
        .env_clear()
        .envs(environment)
        .env(MODE, "1")
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    if let Ok(mut slot) = child.lock() {
        *slot = Some(process.id());
    }
    let mut errors = process.stderr.take().expect("stderr");
    let drain = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = errors.read_to_end(&mut buffer);
        buffer
    });
    if let Some(mut input) = process.stdin.take() {
        let _ = input.write_all(request);
    }
    let mut output = Vec::new();
    let _ = process.stdout.take().expect("stdout").read_to_end(&mut output);
    let status = process.wait().map_err(|error| error.to_string())?;
    if let Ok(mut slot) = child.lock() {
        *slot = None;
    }
    let stderr = String::from_utf8_lossy(&drain.join().unwrap_or_default()).into_owned();
    let frame = if output.len() >= FRAME.len() {
        (0..=output.len() - FRAME.len()).rev().find(|&at| output[at..].starts_with(FRAME))
    } else {
        None
    };
    let (true, Some(at)) = (status.success(), frame) else {
        let code = status.code().unwrap_or(1);
        return Ok(Err(if stderr.is_empty() {
            format!("spawn worker exited with status {code}")
        } else {
            stderr.trim().to_string()
        }));
    };
    // Печать действия до рамки — его stdout.
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(&output[..at]);
    let _ = stdout.flush();
    if !stderr.is_empty() {
        let mut stderr_stream = std::io::stderr();
        let _ = stderr_stream.write_all(stderr.as_bytes());
        let _ = stderr_stream.flush();
    }
    let body = &output[at + FRAME.len()..];
    let mut reader = Reader { bytes: body, at: 0 };
    let ok = reader.take(1)?[0] != 0;
    if ok {
        return Ok(Ok(body[1..].to_vec()));
    }
    Ok(Err(reader.text()?))
}

/// Обслужить запрос родителя и завершить процесс; `main` не идёт.
/// `execute` исполняет вход по символу: значение или текст отказа
/// (`runtime: …`).
pub fn serve(execute: impl FnOnce(&str, Vec<Value>) -> Result<Value, String>, type_name: TypeName<'_>) -> ! {
    crate::host::set_environment(MODE, None);
    let mut response = FRAME.to_vec();
    let outcome = (|| -> Result<Vec<u8>, String> {
        let mut raw = Vec::new();
        std::io::stdin().read_to_end(&mut raw).map_err(|error| format!("runtime: {error}"))?;
        let mut reader = Reader { bytes: &raw, at: 0 };
        let cwd = reader.text().map_err(|error| format!("runtime: {error}"))?;
        crate::host::set_cwd(cwd.into());
        let target = reader.text().map_err(|error| format!("runtime: {error}"))?;
        let count = reader.u32().map_err(|error| format!("runtime: {error}"))?;
        let args = (0..count).map(|_| reader.value()).collect::<Result<Vec<_>, _>>().map_err(|error| format!("runtime: {error}"))?;
        let value = execute(&target, args)?;
        let mut encoded = Vec::new();
        encode(&mut encoded, &value, type_name).map_err(|failure| format!("runtime: RuntimeError: {}", failure.message()))?;
        Ok(encoded)
    })();
    match outcome {
        Ok(value) => {
            response.push(1);
            response.extend(value);
        }
        Err(message) => {
            response.push(0);
            text(&mut response, &message);
        }
    }
    let mut stdout = std::io::stdout();
    let written = stdout.write_all(&response).and_then(|()| stdout.flush());
    std::process::exit(if written.is_ok() { 0 } else { 70 });
}
