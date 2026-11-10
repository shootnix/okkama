//! Process workers (`worker.spawn`, `worker.bind`): a synchronous action runs
//! in a child `sparrow-next --worker FILE.spb`; arguments and the result cross
//! as OKW1 transfer values (the previous engine's wire format).
//!
//! Core words carry no tags, so values are encoded and decoded by the static
//! types of the entry function's parameters and result. Only primitives and
//! arrays and maps of them cross; traversal uses work buffers, never Rust
//! recursion. The Future of a spawn is a driver frame of a synthetic async
//! intrinsic (`program::WORKER`) whose driver polls the child process.
#![allow(unsafe_code)]

use crate::drivers::Step;
use crate::heap::{Integer, Payload};
use crate::ops::MapValue;
use crate::primitives::State;
use crate::program::Kind;
use crate::runtime::{Ctx, FailureKind, Runtime};
use crate::word::{Repr, Word};
use crate::worker_process::{Failure, Output, Request};
use okkama_sparrow::{Type, TypeRef};
use std::io::Write;
use std::rc::Rc;

const FRAME: &[u8; 4] = b"OKW1";
const TRAILER: &[u8; 4] = b"OKL1";
const NOT_TRANSFERABLE: &str = "spawn cannot transfer objects, closures, Futures or native handles";

fn text(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    count(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn count(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "spawn value exceeds wire length".to_owned())?;
    out.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

/// Append `value` of static type `ty`.
fn encode(runtime: &Runtime, out: &mut Vec<u8>, value: Word, ty: TypeRef) -> Result<(), String> {
    let program = &runtime.loaded.program;
    let mut work = vec![(value, ty)];
    while let Some((value, ty)) = work.pop() {
        match program.ty(ty) {
            Some(Type::Int) => {
                out.push(1);
                let digits = match runtime.heap.integer(value) {
                    Integer::Small(n) => n.to_string(),
                    Integer::Big(n) => n.to_string(),
                };
                text(out, &digits)?;
            }
            Some(Type::Float) => {
                out.push(2);
                out.extend_from_slice(&value.to_be_bytes());
            }
            Some(Type::Str) => {
                out.push(3);
                text(out, runtime.heap.text(value))?;
            }
            Some(Type::Char) => {
                out.push(4);
                out.extend_from_slice(&(value as u32).to_be_bytes());
            }
            Some(Type::Byte) => {
                out.push(5);
                out.push(value as u8);
            }
            Some(Type::Bool) => {
                out.push(6);
                out.push(u8::from(value != 0));
            }
            Some(Type::Array(element)) => {
                let elements = runtime.heap.elements(value);
                out.push(7);
                count(out, elements.len())?;
                work.extend(elements.iter().rev().map(|&e| (e, *element)));
            }
            Some(Type::Map(key, item)) => {
                let map = runtime.heap.payload::<MapValue>(value).ok_or_else(|| NOT_TRANSFERABLE.to_owned())?;
                out.push(8);
                count(out, map.entries.len())?;
                let entries: Vec<(Word, Word)> = map.entries.values().copied().collect();
                for &(k, v) in entries.iter().rev() {
                    work.push((v, *item));
                    work.push((k, *key));
                }
            }
            _ => return Err(NOT_TRANSFERABLE.into()),
        }
    }
    Ok(())
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(count)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| "truncated spawn value".to_owned())?;
        let value = &self.bytes[self.at..end];
        self.at = end;
        Ok(value)
    }
    fn count(&mut self) -> Result<usize, String> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("four bytes")) as usize)
    }
    fn text(&mut self) -> Result<&'a str, String> {
        let count = self.count()?;
        std::str::from_utf8(self.take(count)?).map_err(|_| "invalid spawn text".to_owned())
    }
    fn finish(&self) -> Result<(), String> {
        if self.at == self.bytes.len() { Ok(()) } else { Err("trailing spawn data".into()) }
    }

    /// One value of static type `ty`; its tags must match the type.
    fn value(&mut self, runtime: &mut Runtime, ty: TypeRef) -> Result<Word, String> {
        struct Container {
            ty: TypeRef,
            length: usize,
            values: Vec<Word>,
        }
        let mismatch = || "spawn value does not match the declared type".to_owned();
        let mut stack: Vec<Container> = Vec::new();
        let mut expected = ty;
        loop {
            let tag = self.take(1)?[0];
            let program = &runtime.loaded.program;
            let declared = program.ty(expected).cloned();
            let mut value = match (tag, declared) {
                (1, Some(Type::Int)) => {
                    let text = self.text()?;
                    let digits = text.strip_prefix('-').unwrap_or(text);
                    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return Err("invalid spawn integer".into());
                    }
                    let n = num_bigint::BigInt::parse_bytes(text.as_bytes(), 10)
                        .ok_or_else(|| "invalid spawn integer".to_owned())?;
                    runtime.heap.big(n)
                }
                (2, Some(Type::Float)) => {
                    let bits = u64::from_be_bytes(self.take(8)?.try_into().expect("eight bytes"));
                    if !f64::from_bits(bits).is_finite() {
                        return Err("invalid spawn float".into());
                    }
                    bits
                }
                (3, Some(Type::Str)) => {
                    let text = self.text()?;
                    runtime.heap.allocate_text(text)
                }
                (4, Some(Type::Char)) => {
                    let bits = self.count()? as u32;
                    char::from_u32(bits).ok_or_else(|| "invalid spawn character".to_owned())?;
                    u64::from(bits)
                }
                (5, Some(Type::Byte)) => u64::from(self.take(1)?[0]),
                (6, Some(Type::Bool)) => match self.take(1)?[0] {
                    flag @ (0 | 1) => u64::from(flag),
                    _ => return Err("invalid spawn boolean".into()),
                },
                (7, Some(Type::Array(_))) | (8, Some(Type::Map(..))) => {
                    let pairs = if tag == 8 { 2 } else { 1 };
                    // Every value takes at least one byte: reject lengths the
                    // remaining input cannot hold before reserving anything.
                    let length = self
                        .count()?
                        .checked_mul(pairs)
                        .filter(|&n| n <= self.bytes.len() - self.at)
                        .ok_or_else(|| "truncated spawn container".to_owned())?;
                    if length > 0 {
                        stack.push(Container { ty: expected, length, values: Vec::new() });
                        expected = element_type(runtime, expected, 0);
                        continue;
                    }
                    build(runtime, expected, &[])
                }
                (0..=8, _) => return Err(mismatch()),
                _ => return Err(format!("invalid spawn value tag {tag}")),
            };
            loop {
                let Some(container) = stack.last_mut() else {
                    return Ok(value);
                };
                container.values.push(value);
                if container.values.len() < container.length {
                    expected = element_type(runtime, container.ty, container.values.len());
                    break;
                }
                let container = stack.pop().expect("completed container");
                value = build(runtime, container.ty, &container.values);
            }
        }
    }
}

/// The type of the `index`th value inside a container of type `ty` (maps
/// alternate keys and values).
fn element_type(runtime: &Runtime, ty: TypeRef, index: usize) -> TypeRef {
    match runtime.loaded.program.ty(ty) {
        Some(Type::Array(element)) => *element,
        Some(Type::Map(key, item)) => {
            if index.is_multiple_of(2) {
                *key
            } else {
                *item
            }
        }
        _ => unreachable!("containers are arrays or maps"),
    }
}

fn build(runtime: &mut Runtime, ty: TypeRef, values: &[Word]) -> Word {
    let program = &runtime.loaded.program;
    match program.ty(ty).cloned() {
        Some(Type::Array(element)) => match Kind::of(program, element) {
            Kind::Byte => {
                let bytes: Vec<u8> = values.iter().map(|&b| b as u8).collect();
                runtime.heap.allocate_bytes(&bytes)
            }
            kind => runtime.heap.allocate_array(kind.repr(), values),
        },
        Some(Type::Map(key, item)) => {
            let key_kind = Kind::of(program, key);
            let key_type = program.ty(key).cloned();
            let mut map = MapValue {
                key: key_kind.repr(),
                value: Kind::of(program, item).repr(),
                entries: Default::default(),
                owned: Default::default(),
            };
            for pair in values.chunks_exact(2) {
                let k = runtime.key(pair[0], key_kind, key_type.as_ref());
                map.entries.insert(k, (pair[0], pair[1]));
            }
            runtime.heap.allocate_payload(Box::new(map))
        }
        _ => unreachable!("containers are arrays or maps"),
    }
}

/// The receiver fields a bound action (`worker.bind`) passes to its worker
/// entry ahead of the call's arguments.
pub fn bind(runtime: &mut Runtime, action: Word, entry: Word, fields: Vec<(Word, Repr)>) -> Word {
    let function = crate::heap::payload(runtime.heap.header_of(action));
    let words = runtime.heap.layouts.closures[function as usize].len();
    let captures: Vec<Word> = (0..words).map(|i| runtime.heap.field(action, i)).collect();
    let bound = runtime.heap.closure_new(function, &captures);
    runtime.state_new(bound, State::WorkerBinding { entry, fields });
    bound
}

#[derive(Debug)]
struct Call {
    request: Rc<Request>,
    result: Option<TypeRef>,
}

impl Payload for Call {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `worker.spawn`: encode the call, start the child and return its Future.
/// `direct` calls `entry` itself; otherwise `entry` is a bound action whose
/// receiver fields precede `arguments`.
pub fn spawn(runtime: &mut Runtime, ctx: *mut Ctx, entry: Word, arguments: &[Word], direct: bool) -> Result<Word, String> {
    if runtime.worker_command.is_empty() {
        return Err("process workers require an explicit worker command".into());
    }
    let (entry, arguments) = if direct {
        (entry, arguments.to_vec())
    } else {
        let Some(State::WorkerBinding { entry, fields }) = runtime.states.get(&entry) else {
            return Err("spawn requires a synchronous bound object action whose receiver fields reconstruct it".into());
        };
        let mut values: Vec<Word> = fields.iter().map(|&(w, _)| w).collect();
        values.extend_from_slice(arguments);
        (*entry, values)
    };
    let function = crate::heap::payload(runtime.heap.header_of(entry)) as usize;
    let source = &runtime.loaded.program.functions[function];
    if source.params.len() != arguments.len() {
        return Err("worker entry cannot capture VM values".into());
    }
    let types: Vec<TypeRef> = source.params.iter().map(|r| source.registers[r.0 as usize]).collect();
    let (symbol, result) = (source.symbol.clone(), source.result);
    let directory = okkama_runtime_core::host::cwd();
    let mut request = Vec::new();
    text(&mut request, &directory.to_string_lossy())?;
    text(&mut request, &symbol)?;
    count(&mut request, arguments.len())?;
    for (&value, &ty) in arguments.iter().zip(&types) {
        encode(runtime, &mut request, value, ty)?;
    }
    let driver = runtime
        .loaded
        .worker(result)
        .ok_or_else(|| "worker.spawn without a worker Future type".to_owned())?;
    let future = runtime.future_new(driver, &[]);
    let request = Request::new(runtime.worker_command.clone(), request, None);
    let object = runtime.heap.allocate_payload(Box::new(Call { request, result }));
    let slot = runtime.driver_slot(future);
    crate::tasks::set(future, slot, object);
    // A spawn starts its worker now.
    runtime.spawn(ctx, future);
    Ok(future)
}

/// The driver of a worker Future: poll the child; `deliver` cancels it (the
/// child is killed and reaped before the Future settles).
pub fn drive(runtime: &mut Runtime, frame: Word, deliver: bool) -> Step {
    let object = crate::tasks::get(frame, runtime.driver_slot(frame));
    let call = runtime.heap.payload::<Call>(object).expect("worker call");
    let (request, result) = (call.request.clone(), call.result);
    if deliver {
        request.cancel();
    }
    let waker = runtime.io.waker(frame);
    let polled = {
        let _tokio = runtime.io.tokio().enter();
        let _local = runtime.io.local().enter();
        request.poll(&mut std::task::Context::from_waker(&waker))
    };
    let outcome = match polled {
        std::task::Poll::Pending => {
            // Keeps the scheduler waiting on external progress.
            runtime.io.requests.insert(frame, u64::MAX);
            return Step::Pending;
        }
        std::task::Poll::Ready(outcome) => outcome,
    };
    runtime.io.requests.remove(&frame);
    let message = match outcome {
        Err(failure) => match &*failure {
            Failure::Cancelled => return Step::Failed(runtime.cancelled_failure()),
            Failure::Process(message) => format!("cannot execute spawn worker: {message}"),
        },
        Ok(output) => match settle(runtime, &output, result) {
            Ok(value) => return Step::Done(value),
            Err(message) => message,
        },
    };
    Step::Failed(runtime.failure_new(FailureKind::Program(message)))
}

/// The child's output: its own stdout before the response frame and its
/// stderr go to ours; the frame holds the result or the failure text.
fn settle(runtime: &mut Runtime, output: &Output, result: Option<TypeRef>) -> Result<Word, String> {
    if !output.success {
        return Err(if output.stderr.is_empty() {
            format!("spawn worker exited with status {}", output.code.unwrap_or(1))
        } else {
            String::from_utf8_lossy(&output.stderr).trim().into()
        });
    }
    let raw = &output.stdout;
    if raw.len() < 17 || !raw.ends_with(TRAILER) {
        return Err("invalid worker response trailer".into());
    }
    let end = raw.len() - 12;
    let length = u64::from_be_bytes(raw[end..end + 8].try_into().expect("eight bytes"));
    let start = usize::try_from(length)
        .ok()
        .and_then(|length| end.checked_sub(length))
        .ok_or_else(|| "invalid worker response length".to_owned())?;
    if !raw[start..end].starts_with(FRAME) {
        return Err("invalid worker response frame".into());
    }
    let mut stdout = std::io::stdout();
    stdout.write_all(&raw[..start]).and_then(|()| stdout.flush()).map_err(|e| e.to_string())?;
    std::io::stderr().write_all(&output.stderr).map_err(|e| e.to_string())?;
    let mut reader = Reader::new(&raw[start + 4..end]);
    match reader.take(1)?[0] {
        1 => {
            let value = match result {
                Some(ty) => reader.value(runtime, ty)?,
                None => {
                    // A void entry answers with an unbound value.
                    if reader.take(1)?[0] != 0 {
                        return Err("invalid worker response value".into());
                    }
                    crate::word::UNBOUND
                }
            };
            reader.finish()?;
            Ok(value)
        }
        0 => {
            let message = reader.text()?.to_owned();
            reader.finish()?;
            Err(message)
        }
        _ => Err("invalid worker response status".into()),
    }
}

/// The child side: run the requested entry over the decoded arguments and
/// frame its result (or failure text) for the parent.
pub fn respond(runtime: &mut Box<Runtime>, request: &[u8], stack: u64) -> Vec<u8> {
    let outcome = (|| -> Result<Vec<u8>, String> {
        let mut reader = Reader::new(request);
        let directory = reader.text()?;
        let target = reader.text()?;
        let program = &runtime.loaded.program;
        let entry = program
            .functions
            .iter()
            .position(|f| f.symbol == target)
            .ok_or_else(|| "unknown worker entry".to_owned())?;
        let source = &program.functions[entry];
        let types: Vec<TypeRef> = source.params.iter().map(|r| source.registers[r.0 as usize]).collect();
        let result = source.result;
        if reader.count()? != types.len() {
            return Err("worker argument count mismatch".into());
        }
        let mut arguments = Vec::with_capacity(types.len());
        for ty in types {
            arguments.push(reader.value(runtime, ty)?);
        }
        reader.finish()?;
        let path = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
        if !path.is_dir() {
            return Err("worker working directory is not a directory".into());
        }
        okkama_runtime_core::host::set_cwd(path);
        let value = runtime.run_function_with(entry, &arguments, stack).map_err(|e| e.0)?;
        let mut bytes = Vec::new();
        match result {
            Some(ty) => encode(runtime, &mut bytes, value, ty)?,
            None => bytes.push(0),
        }
        Ok(bytes)
    })();
    let mut response = FRAME.to_vec();
    match outcome {
        Ok(value) => {
            response.push(1);
            response.extend(value);
        }
        Err(message) => {
            response.push(0);
            // A failure text longer than the wire allows is cut, not lost.
            let message: String = message.chars().take(1 << 20).collect();
            let _ = text(&mut response, &message);
        }
    }
    let length = response.len() as u64;
    response.extend_from_slice(&length.to_be_bytes());
    response.extend_from_slice(TRAILER);
    response
}
