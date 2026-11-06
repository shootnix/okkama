//! Runtime-provided functions (SPB `intrinsic`). Synchronous ones run here;
//! async ones are driven by `drivers`. Each function's intrinsic is resolved
//! once when the program loads.
#![allow(unsafe_code)]
use crate::primitives::{self, State};
use crate::program::Kind;
use crate::runtime::{Ctx, FAILED, FailureKind, OK, Runtime};
use crate::word::{Repr, UNBOUND, Word};
use okkama_sparrow::Type;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    /// The Future of a process worker (`worker`).
    Worker,
    Ordering,
    AtomicNew,
    AtomicValue,
    AtomicReplace,
    AtomicCompareReplace,
    AtomicClosed,
    AtomicClose,
    CancellationNew,
    TokenNew,
    Cancelling,
    Cancel,
    TokenRequested,
    TokenCheck,
    TokenWait,
    EventNew,
    EventNewReady,
    EventReady,
    EventSet,
    EventWait,
    LockNew,
    LockGuard,
    LockLocked,
    GrantClosed,
    GrantClose,
    SemaphorePermits,
    SemaphorePermit,
    BarrierWait,
    BarrierState,
    BarrierAbort,
    BarrierReset,
    BarrierRead(usize),
    ChannelNew,
    ChannelEndpoint,
    EndpointNew,
    Send,
    Receive,
    EndpointClose,
    GroupNew,
    GroupStart,
    GroupActive,
    GroupCancel,
    GroupWait,
    ScopeResult,
    CollectAll,
    CollectRace,
    CollectFirst,
    TimeoutBefore,
    TimeoutWithin,
    StreamRemaining,
    StreamCompletion,
    FakeSet,
    FakeGet,
    FakeHas,
    FakeRemove,
    FakeClear,
    FakeSize,
    TestOk,
    TestIs,
    TestIsnt,
    TestFail,
}

impl Intrinsic {
    pub fn resolve(name: &str) -> Option<Self> {
        use Intrinsic::*;
        Some(match name {
            crate::program::WORKER => Worker,
            "okkama.array.ordering" => Ordering,
            "okkama.atomic.Atomic.new_value" => AtomicNew,
            "okkama.atomic.Atomic.value" => AtomicValue,
            "okkama.atomic.Atomic.replace" => AtomicReplace,
            "okkama.atomic.Atomic.compare_and_replace" => AtomicCompareReplace,
            "okkama.atomic.Atomic.closed" => AtomicClosed,
            "okkama.atomic.Atomic.close" => AtomicClose,
            "okkama.cancellation.Cancellation.new" => CancellationNew,
            "okkama.cancellation.CancellationToken.new" => TokenNew,
            "okkama.cancellation.Cancellation.cancelling" => Cancelling,
            "okkama.cancellation.Cancellation.cancel" => Cancel,
            "okkama.cancellation.CancellationToken.requested" => TokenRequested,
            "okkama.cancellation.CancellationToken.check" => TokenCheck,
            "okkama.cancellation.CancellationToken.wait" => TokenWait,
            "okkama.event.Event.new" => EventNew,
            "okkama.event.Event.new_ready" => EventNewReady,
            "okkama.event.Event.ready" => EventReady,
            "okkama.event.Event.set" => EventSet,
            "okkama.event.Event.wait" => EventWait,
            "okkama.lock.Lock.new" => LockNew,
            "okkama.lock.Lock.guard" => LockGuard,
            "okkama.lock.Lock.locked" => LockLocked,
            "okkama.lock.LockGuard.closed" | "okkama.semaphore.Permit.closed" => GrantClosed,
            "okkama.lock.LockGuard.close" | "okkama.semaphore.Permit.close" => GrantClose,
            "okkama.semaphore.Semaphore.permits" => SemaphorePermits,
            "okkama.semaphore.Semaphore.permit" => SemaphorePermit,
            "okkama.barrier.Barrier.wait" => BarrierWait,
            "okkama.barrier.Barrier.state" => BarrierState,
            "okkama.barrier.Barrier.abort" => BarrierAbort,
            "okkama.barrier.Barrier.reset" => BarrierReset,
            "okkama.barrier.BarrierState.parties" => BarrierRead(0),
            "okkama.barrier.BarrierState.waiters" => BarrierRead(1),
            "okkama.barrier.BarrierState.generation" => BarrierRead(2),
            "okkama.barrier.BarrierState.broken" => BarrierRead(3),
            "okkama.channel.Channel.new_capacity" => ChannelNew,
            "okkama.channel.Channel.sending" | "okkama.channel.Channel.receiving" => ChannelEndpoint,
            "okkama.channel.Sending.new" | "okkama.channel.Receiving.new" => EndpointNew,
            "okkama.channel.Sending.send" => Send,
            "okkama.channel.Receiving.item" => Receive,
            "okkama.channel.Sending.close" | "okkama.channel.Receiving.close" => EndpointClose,
            "okkama.scope.FutureGroup.new" => GroupNew,
            "okkama.scope.FutureGroup.start" => GroupStart,
            "okkama.scope.FutureGroup.active" => GroupActive,
            "okkama.scope.FutureGroup.cancel" => GroupCancel,
            "okkama.scope.FutureGroup.wait" => GroupWait,
            "okkama.scope.FutureScope.result" => ScopeResult,
            "okkama.collect.AllFutures.results" => CollectAll,
            "okkama.collect.Raced.result" => CollectRace,
            "okkama.collect.FirstSuccessful.result" => CollectFirst,
            "okkama.timeout.Before.result" => TimeoutBefore,
            "okkama.timeout.Within.result" => TimeoutWithin,
            "okkama.collect.FutureCompletions.remaining_count" => StreamRemaining,
            "okkama.collect.FutureCompletions.completion" => StreamCompletion,
            "okkama.fake.storage.set" => FakeSet,
            "okkama.fake.storage.get" => FakeGet,
            "okkama.fake.storage.has" => FakeHas,
            "okkama.fake.storage.remove" => FakeRemove,
            "okkama.fake.storage.clear" => FakeClear,
            "okkama.fake.storage.size" => FakeSize,
            "okkama.test.ok" => TestOk,
            "okkama.test.is" => TestIs,
            "okkama.test.isnt" => TestIsnt,
            "okkama.test.fail" => TestFail,
            _ => return None,
        })
    }
}

/// Outcome of a synchronous intrinsic.
enum Outcome {
    Value(Word, Repr),
    Fail(FailureKind),
    /// An existing failure (from a call) continues.
    Raise(Word),
    Fatal(String),
}
use Outcome::{Fail, Fatal, Raise, Value};

const VOID: Outcome = Value(UNBOUND, Repr::Raw);

fn program_failure(message: &str) -> Outcome {
    Fail(FailureKind::Program(message.into()))
}

/// Call intrinsic `function` with `count` argument words at `arguments`.
///
/// SAFETY: called from machine code with its context.
#[inline(never)]
pub extern "C" fn rt_intrinsic(ctx: *mut Ctx, function: u64, arguments: *const Word, count: u64) -> Word {
    let frame = crate::runtime::frame_pointer!();
    let runtime = unsafe { &mut *(*ctx).runtime };
    let mut words = [UNBOUND; 4];
    let count = (count as usize).min(words.len());
    words[..count].copy_from_slice(unsafe { std::slice::from_raw_parts(arguments, count) });
    let function = function as usize;
    runtime.count(|r| format!("intrinsic {}", r.loaded.program.functions[function].symbol));
    let outcome = match runtime.intrinsics[function] {
        Some(Intrinsic::Ordering) => {
            let result = ordering(ctx, runtime, function, words[0]);
            Value(result, Repr::Ref)
        }
        Some(intrinsic) => call(ctx, runtime, function, intrinsic, &words[..count]),
        None => {
            let name = runtime.loaded.program.functions[function].intrinsic.as_ref().map_or("", |i| i.name.as_str());
            Fatal(format!("intrinsic {name} is not available in sparrow-core yet"))
        }
    };
    let ctx = unsafe { &mut *ctx };
    match outcome {
        Value(result, repr) => {
            if ctx.status == OK {
                runtime.safepoint(frame, ctx, &[(result, repr)]);
            }
            result
        }
        Fail(kind) => {
            ctx.failure = runtime.failure_new(kind);
            ctx.status = FAILED;
            0
        }
        Raise(failure) => {
            ctx.failure = failure;
            ctx.status = FAILED;
            0
        }
        Fatal(message) => crate::runtime::fatal(ctx, runtime, message),
    }
}

/// The object descriptors an intrinsic names (results and thrown errors).
fn related(runtime: &Runtime, function: usize, index: usize) -> u32 {
    runtime.loaded.program.functions[function].intrinsic.as_ref().expect("intrinsic").objects[index].0
}

/// A thrown instance of the `index`th related object.
pub(crate) fn thrown(runtime: &mut Runtime, function: usize, index: usize, fields: &[Word]) -> FailureKind {
    let object = raised(runtime, related(runtime, function, index), fields);
    FailureKind::Thrown(object)
}

/// An error object the runtime raises, from its payload `values`. An error
/// enum lowers to `[$tag, $<case>_<name>: array…]`; the runtime raises its
/// first case, whose payload values go into one-element arrays.
fn raised(runtime: &mut Runtime, descriptor: u32, values: &[Word]) -> Word {
    let program = &runtime.loaded.program;
    let fields = &program.objects[descriptor as usize].fields;
    if fields.first().is_none_or(|(name, _)| name != "$tag") {
        return runtime.heap.object_new(descriptor, values);
    }
    let shapes: Vec<(bool, Repr)> = fields[1..]
        .iter()
        .map(|(name, ty)| {
            let element = match program.ty(*ty) {
                Some(Type::Array(element)) => Kind::of(program, *element).repr(),
                _ => Repr::Ref,
            };
            (name.starts_with("$0_"), element)
        })
        .collect();
    let mut payload = values.iter();
    let mut words = vec![runtime.heap.int(0)];
    for (first, element) in shapes {
        let elements: Vec<Word> = if first { payload.next().copied().into_iter().collect() } else { Vec::new() };
        words.push(runtime.heap.allocate_array(element, &elements));
    }
    runtime.heap.object_new(descriptor, &words)
}

fn boolean(value: bool) -> Outcome {
    Value(Word::from(value), Repr::Raw)
}

fn call(ctx: *mut Ctx, runtime: &mut Runtime, function: usize, intrinsic: Intrinsic, args: &[Word]) -> Outcome {
    use Intrinsic::*;
    let new = |runtime: &mut Runtime, fields: &[Word]| runtime.heap.object_new(related(runtime, function, 0), fields);
    match intrinsic {
        // An Atomic is a heap cell: word 1 its value, word 2 closed (0 open, 1
        // closed). Machine code reads and writes it inline (see `jit`); here
        // are the calls through roles and the closed and equality cases.
        AtomicNew => Value(new(runtime, &[args[0], 0]), Repr::Ref),
        AtomicClosed => boolean(runtime.heap.field(args[0], 1) != 0),
        AtomicClose => {
            runtime.heap.set_field(args[0], 1, 1);
            runtime.heap.set_field(args[0], 0, UNBOUND);
            VOID
        }
        AtomicValue | AtomicReplace | AtomicCompareReplace => {
            if runtime.heap.field(args[0], 1) != 0 {
                return Fail(thrown(runtime, function, 0, &[]));
            }
            let current = runtime.heap.field(args[0], 0);
            match intrinsic {
                AtomicValue => {
                    let repr = runtime.loaded.functions[function].result.map_or(Repr::Raw, Kind::repr);
                    Value(current, repr)
                }
                AtomicReplace => {
                    runtime.heap.set_field(args[0], 0, args[1]);
                    VOID
                }
                _ => {
                    let f = &runtime.loaded.program.functions[function];
                    let ty = f.registers[f.params[1].0 as usize];
                    let equal = match crate::ops::equal(runtime, current, args[1], ty) {
                        Ok(equal) => equal,
                        Err(message) => return Fatal(message),
                    };
                    if equal {
                        runtime.heap.set_field(args[0], 0, args[2]);
                    }
                    boolean(equal)
                }
            }
        }
        CancellationNew => {
            let object = new(runtime, &[]);
            runtime.state_new(object, State::Event(Rc::default()));
            Value(object, Repr::Ref)
        }
        TokenNew => Value(new(runtime, &[]), Repr::Ref),
        Cancelling => {
            let Some(event) = runtime.event_of(args[0]) else {
                return program_failure("Invalid cancellation capability");
            };
            let crate::primitives::EventRef::Shared(event) = event else {
                return program_failure("Invalid cancellation capability");
            };
            let token = new(runtime, &[]);
            runtime.state_new(token, State::Event(event));
            Value(token, Repr::Ref)
        }
        Cancel | EventSet => {
            let Some(event) = runtime.event_of(args[0]) else {
                return program_failure(if intrinsic == Cancel { "Invalid cancellation capability" } else { "Invalid async event" });
            };
            runtime.event_set(&event);
            VOID
        }
        TokenRequested | TokenCheck | EventReady => {
            let Some(event) = runtime.event_of(args[0]) else {
                return program_failure(if intrinsic == EventReady { "Invalid async event" } else { "Invalid cancellation capability" });
            };
            let set = runtime.event_ready(&event);
            match intrinsic {
                TokenCheck if set => Fail(FailureKind::Cancelled),
                TokenCheck => VOID,
                _ => boolean(set),
            }
        }
        EventNew | EventNewReady => {
            let ready = intrinsic == EventNewReady && args[0] != 0;
            // An Event is a cell: `initial`, the set flag, its waiters.
            let object = new(runtime, &[Word::from(ready), Word::from(ready), UNBOUND]);
            Value(object, Repr::Ref)
        }
        LockNew => {
            let object = new(runtime, &[]);
            runtime.state_new(object, State::Pool(Rc::new(RefCell::new(primitives::Pool { remaining: 1, ..Default::default() }))));
            Value(object, Repr::Ref)
        }
        LockLocked => match runtime.pool_of(args[0], false) {
            Some(pool) => boolean(pool.borrow().remaining <= 0),
            None => program_failure("Invalid async lock"),
        },
        SemaphorePermits => match runtime.pool_of(args[0], true) {
            Some(pool) => Value(runtime.heap.int(Runtime::pool_available(&pool.borrow())), Repr::Int),
            None => program_failure("Cannot read field amount"),
        },
        GrantClosed | GrantClose => {
            let Some(State::Grant { pool, closed }) = runtime.states.get_mut(&args[0]) else {
                return program_failure("Invalid async lock guard");
            };
            if intrinsic == GrantClosed {
                return boolean(*closed);
            }
            if !std::mem::replace(closed, true) {
                let pool = pool.clone();
                runtime.pool_release(&pool);
            }
            VOID
        }
        BarrierState | BarrierAbort | BarrierReset => {
            let Some(barrier) = runtime.barrier_of(args[0]) else {
                return program_failure("Cannot read field amount");
            };
            match intrinsic {
                BarrierState => {
                    let snapshot = {
                        let b = barrier.borrow();
                        [b.parties, b.waiting.len() as i64, b.generation, i64::from(b.broken)]
                    };
                    let object = new(runtime, &[]);
                    runtime.state_new(object, State::Snapshot(snapshot));
                    Value(object, Repr::Ref)
                }
                BarrierAbort => {
                    runtime.barrier_break(&barrier);
                    VOID
                }
                _ => {
                    runtime.barrier_break(&barrier);
                    let mut b = barrier.borrow_mut();
                    b.generation += 1;
                    b.broken = false;
                    VOID
                }
            }
        }
        BarrierRead(field) => {
            let Some(State::Snapshot(snapshot)) = runtime.states.get(&args[0]) else {
                return program_failure("Invalid barrier state");
            };
            let value = snapshot[field];
            if field == 3 {
                boolean(value != 0)
            } else {
                Value(runtime.heap.int(value), Repr::Int)
            }
        }
        ChannelNew => {
            let Some(capacity) = runtime.small_or_big(args[0]).filter(|&n| n >= 0) else {
                if runtime.big_value(args[0]).sign() == num_bigint::Sign::Minus {
                    return Fail(thrown(runtime, function, 1, &[args[0]]));
                }
                return program_failure("Channel capacity is too large");
            };
            if capacity > i64::from(i32::MAX) {
                return program_failure("Channel capacity is too large");
            }
            let object = new(runtime, &[]);
            let channel = primitives::Channel {
                capacity: capacity as usize,
                closed: false,
                repr: Repr::Raw,
                messages: Default::default(),
                senders: Default::default(),
                receivers: Default::default(),
            };
            runtime.state_new(object, State::Channel(Rc::new(RefCell::new(channel))));
            Value(object, Repr::Ref)
        }
        ChannelEndpoint => {
            let Some(channel) = runtime.channel_of(args[0]) else {
                return program_failure("Invalid channel endpoint");
            };
            let object = new(runtime, &[]);
            runtime.state_new(object, State::Channel(channel));
            Value(object, Repr::Ref)
        }
        EndpointNew => Value(new(runtime, &[]), Repr::Ref),
        EndpointClose => {
            let Some(channel) = runtime.channel_of(args[0]) else {
                return program_failure("Invalid channel endpoint");
            };
            runtime.channel_close(&channel);
            VOID
        }
        GroupNew => {
            let object = new(runtime, &[]);
            runtime.state_new(object, State::Group(primitives::Group::default()));
            Value(object, Repr::Ref)
        }
        GroupStart => {
            let (receiver, future) = (args[0], args[1]);
            let Some(group) = runtime.group_mut(receiver) else {
                return program_failure("Invalid future group");
            };
            let known = group.entries.contains(&future);
            if group.sealed && !known {
                return Fail(thrown(runtime, function, 0, &[]));
            }
            if !known {
                group.entries.push(future);
                group.owned.push(false);
            }
            runtime.spawn(ctx, future);
            VOID
        }
        GroupActive => {
            let Some(group) = runtime.group_mut(args[0]) else {
                return program_failure("Invalid future group");
            };
            let active = group
                .entries
                .iter()
                .filter(|&&e| matches!(crate::tasks::get(e, crate::tasks::STATE), crate::tasks::COLD | crate::tasks::RUNNING))
                .count();
            Value(runtime.heap.int(active as i64), Repr::Int)
        }
        GroupCancel => {
            if runtime.group_mut(args[0]).is_none() {
                return program_failure("Invalid future group");
            }
            runtime.group_cancel(ctx, args[0]);
            VOID
        }
        StreamRemaining | StreamCompletion => {
            if let Err(outcome) = stream_init(ctx, runtime, function, args[0]) {
                return outcome;
            }
            if intrinsic == StreamCompletion {
                // Origins are read at the call; the request itself starts cold.
                return Value(runtime.future_new(function, args), Repr::Ref);
            }
            runtime.stream_poll(args[0], UNBOUND);
            let stream = runtime.stream_mut(args[0]).expect("stream");
            let remaining = (stream.entries.len() - stream.delivered) as i64;
            Value(runtime.heap.int(remaining), Repr::Int)
        }
        FakeSet | FakeGet | FakeHas | FakeRemove | FakeClear | FakeSize => fake_storage(runtime, function, intrinsic, args),
        TestOk | TestIs | TestIsnt | TestFail => {
            if runtime.host.as_ref().is_none_or(|h| h.test_failures.is_none()) {
                return program_failure("The test capability is available only inside a running test");
            }
            let message = match intrinsic {
                TestOk => (args[0] == 0).then(|| "expected true, got false".to_owned()),
                TestFail => Some(runtime.heap.text(args[0]).to_owned()),
                _ => {
                    let f = &runtime.loaded.program.functions[function];
                    let ty = f.registers[f.params[0].0 as usize];
                    let equal = match crate::ops::equal(runtime, args[0], args[1], ty) {
                        Ok(equal) => equal,
                        Err(message) => return Fatal(message),
                    };
                    if intrinsic == TestIs && !equal {
                        Some(format!("expected {}, got {}", show(runtime, args[1], ty), show(runtime, args[0], ty)))
                    } else if intrinsic == TestIsnt && equal {
                        Some(format!("expected values to differ, both were {}", show(runtime, args[0], ty)))
                    } else {
                        None
                    }
                }
            };
            if let Some(failures) = runtime.host.as_mut().and_then(|h| h.test_failures.as_mut()) {
                failures.extend(message);
            }
            VOID
        }
        _ => Fatal(format!("intrinsic {intrinsic:?} is async")),
    }
}

/// Per-fake storage of test fakes (`fake.storage.*`): typed values by key,
/// owned by the fake object.
type FakeMap = std::collections::BTreeMap<crate::ops::Key, (Word, Word, okkama_sparrow::TypeRef, Repr)>;

fn fake_map(runtime: &mut Runtime, receiver: Word) -> Option<&mut FakeMap> {
    match runtime.states.get_mut(&receiver) {
        Some(State::FakeStorage(storage)) => Some(storage),
        _ => None,
    }
}

fn fake_storage(runtime: &mut Runtime, function: usize, intrinsic: Intrinsic, args: &[Word]) -> Outcome {
    use Intrinsic::*;
    let receiver = args[0];
    let header = runtime.heap.header_of(receiver);
    if crate::heap::kind(header) != crate::heap::OBJECT
        || !runtime.loaded.program.objects[crate::heap::payload(header) as usize].name.ends_with("$fake")
    {
        return program_failure("fake storage requires a role fake");
    }
    match intrinsic {
        FakeClear => {
            if let Some(storage) = fake_map(runtime, receiver) {
                storage.clear();
            }
            return VOID;
        }
        FakeSize => {
            let size = fake_map(runtime, receiver).map_or(0, |s| s.len());
            return Value(runtime.heap.int(size as i64), Repr::Int);
        }
        _ => {}
    }
    let (params, result) = {
        let f = &runtime.loaded.program.functions[function];
        (f.params.iter().map(|r| r.0 as usize).collect::<Vec<_>>(), f.result)
    };
    let registers = &runtime.loaded.program.functions[function].registers;
    let key_type = registers[params[1]];
    let key_kind = runtime.loaded.functions[function].kinds[params[1]];
    let key_ty = runtime.loaded.program.ty(key_type).cloned();
    let indexed = runtime.key(args[1], key_kind, key_ty.as_ref());
    if intrinsic == FakeSet {
        let value_type = runtime.loaded.program.functions[function].registers[params[2]];
        let repr = runtime.loaded.functions[function].kinds[params[2]].repr();
        if fake_map(runtime, receiver).is_none() {
            runtime.state_new(receiver, State::FakeStorage(Default::default()));
        }
        fake_map(runtime, receiver).expect("storage").insert(indexed, (args[1], args[2], value_type, repr));
        return VOID;
    }
    let found = fake_map(runtime, receiver).and_then(|s| s.get(&indexed)).copied();
    if intrinsic == FakeHas {
        return boolean(found.is_some());
    }
    let Some((_, value, stored, repr)) = found else {
        let shown = show(runtime, args[1], key_type);
        return program_failure(&format!("Fake storage has no key {shown}"));
    };
    if intrinsic == FakeRemove {
        if let Some(storage) = fake_map(runtime, receiver) {
            storage.remove(&indexed);
        }
        return VOID;
    }
    // get: the stored value under the requested type.
    let expected = result.expect("fake get result");
    let result_repr = runtime.loaded.functions[function].result.map_or(Repr::Raw, Kind::repr);
    let program = &runtime.loaded.program;
    if program.ty(expected) == program.ty(stored) {
        return Value(value, result_repr);
    }
    match program.ty(expected).cloned() {
        Some(Type::Any) => {
            let boxed = crate::ops::AnyBox { value, ty: stored, repr };
            Value(runtime.heap.allocate_payload(Box::new(boxed)), Repr::Ref)
        }
        Some(Type::Role(role)) => {
            let header = runtime.heap.header_of(value);
            let does = repr == Repr::Ref
                && crate::heap::kind(header) == crate::heap::OBJECT
                && program.objects[crate::heap::payload(header) as usize].does.iter().any(|w| w.role == role);
            if does {
                Value(value, Repr::Ref)
            } else {
                program_failure("Fake storage value does not match the requested type")
            }
        }
        _ => program_failure("Fake storage value does not match the requested type"),
    }
}

/// A value as test reports show it (strings quoted at the top level).
fn show(runtime: &Runtime, word: Word, ty: okkama_sparrow::TypeRef) -> String {
    let mut out = String::new();
    show_into(runtime, word, ty, true, &mut out);
    out
}

fn show_into(runtime: &Runtime, word: Word, ty: okkama_sparrow::TypeRef, quoted: bool, out: &mut String) {
    let program = &runtime.loaded.program;
    match program.ty(ty) {
        Some(Type::Str) if quoted => {
            out.push('"');
            out.push_str(runtime.heap.text(word));
            out.push('"');
        }
        Some(Type::Str) => out.push_str(runtime.heap.text(word)),
        Some(Type::Bool) => out.push_str(if word == 0 { "false" } else { "true" }),
        Some(Type::Byte) => out.push_str(&(word as u8 as i8).to_string()),
        Some(Type::Char) => out.push_str(&word.to_string()),
        Some(Type::Int) => out.push_str(&runtime.big_value(word).to_string()),
        Some(Type::Float) => out.push_str(&crate::scalar::float_text(f64::from_bits(word))),
        Some(Type::Rat) => out.push_str(&runtime.rational(word).to_string()),
        Some(Type::Enum(e)) => {
            let name = program.enums.get(e.0 as usize).and_then(|en| en.cases.get(word as u32 as usize));
            out.push_str(name.map_or("?", |n| n.as_str()));
        }
        Some(Type::Array(element)) => {
            let element = *element;
            out.push('[');
            for (i, &item) in runtime.heap.elements(word).iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                show_into(runtime, item, element, false, out);
            }
            out.push(']');
        }
        Some(Type::Map(key, value)) => {
            let (key, value) = (*key, *value);
            out.push('{');
            if let Some(map) = runtime.heap.payload::<crate::ops::MapValue>(word) {
                for (i, &(k, v)) in map.entries.values().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    show_into(runtime, k, key, false, out);
                    out.push('=');
                    show_into(runtime, v, value, false, out);
                }
            }
            out.push('}');
        }
        Some(Type::Any) => match runtime.heap.payload::<crate::ops::AnyBox>(word) {
            Some(boxed) => show_into(runtime, boxed.value, boxed.ty, quoted, out),
            None => out.push_str("null"),
        },
        _ if word == UNBOUND => out.push_str("null"),
        _ => {
            let header = runtime.heap.header_of(word);
            let name = if crate::heap::kind(header) == crate::heap::OBJECT {
                program.objects[crate::heap::payload(header) as usize].name.as_str()
            } else {
                "value"
            };
            out.push_str(&format!("{name}@{word:x}"));
        }
    }
}

/// `stream_init` for drivers: Err(Some(failure)), or Err(None) when stopped.
pub(crate) fn stream_ready(ctx: *mut Ctx, runtime: &mut Runtime, function: usize, receiver: Word) -> Result<(), Option<Word>> {
    match stream_init(ctx, runtime, function, receiver) {
        Ok(()) => Ok(()),
        Err(Raise(failure)) => Err(Some(failure)),
        Err(Fail(kind)) => Err(Some(runtime.failure_new(kind))),
        Err(Fatal(message)) => {
            crate::runtime::fatal(unsafe { &mut *ctx }, runtime, message);
            Err(None)
        }
        Err(Value(..)) => Ok(()),
    }
}

/// Read the origins of a FutureCompletions once (again after a failed read).
fn stream_init(ctx: *mut Ctx, runtime: &mut Runtime, function: usize, receiver: Word) -> Result<(), Outcome> {
    if runtime.stream_mut(receiver).is_some() {
        return Ok(());
    }
    let program = &runtime.loaded.program;
    let source = &program.functions[function];
    let Some(Type::Object(owner)) = program.ty(source.registers[source.params[0].0 as usize]) else {
        return Err(Fatal("invalid FutureCompletions receiver".into()));
    };
    let Some(Type::Role(role)) = program.ty(program.objects[owner.0 as usize].fields[0].1) else {
        return Err(Fatal("invalid FutureCompletions origins".into()));
    };
    let role = role.0;
    let methods = &program.roles[role as usize].methods;
    let (Some(size_slot), Some(item_slot)) =
        (methods.iter().position(|m| m.name == "size"), methods.iter().position(|m| m.name == "item"))
    else {
        return Err(Fatal("invalid FutureCompletions origins".into()));
    };
    // The state exists while reading, so the inputs read so far are roots.
    runtime.state_new(receiver, State::Stream(Box::default()));
    let failed = |ctx: *mut Ctx, runtime: &mut Runtime| {
        runtime.states.remove(&receiver);
        let ctx = unsafe { &mut *ctx };
        if ctx.status == FAILED {
            ctx.status = OK;
            return Err(Raise(std::mem::replace(&mut ctx.failure, UNBOUND)));
        }
        Err(Fatal(runtime.fatal.clone().unwrap_or_default()))
    };
    let sequence = runtime.heap.field(receiver, 0);
    let Some(size) = call_role(ctx, runtime, sequence, role, size_slot as u32, &[]) else {
        return failed(ctx, runtime);
    };
    let size = runtime.small_or_big(size).unwrap_or(0).max(0);
    for index in 0..size {
        let position = runtime.heap.int(index);
        let Some(item) = call_role(ctx, runtime, sequence, role, item_slot as u32, &[position]) else {
            return failed(ctx, runtime);
        };
        runtime.stream_mut(receiver).expect("stream").entries.push(item);
    }
    let stream = runtime.stream_mut(receiver).expect("stream");
    stream.emitted = vec![false; stream.entries.len()];
    stream.cancelled = vec![false; stream.entries.len()];
    Ok(())
}

/// Call method `slot` of `role` on `receiver` through its witness; None
/// when it failed or stopped (the context says which).
pub(crate) fn call_role(ctx: *mut Ctx, runtime: &mut Runtime, receiver: Word, role: u32, slot: u32, args: &[Word]) -> Option<Word> {
    let descriptor = crate::heap::payload(runtime.heap.header_of(receiver)) as usize;
    let witness = runtime.loaded.program.objects[descriptor].does.iter().find(|w| w.role.0 == role)?;
    let function = witness.slots[slot as usize].0 as usize;
    let mut words = vec![receiver];
    words.extend_from_slice(args);
    call_function(ctx, runtime, function, &words)
}

/// Call a synchronous function (an async one makes its cold Future).
pub(crate) fn call_function(ctx: *mut Ctx, runtime: &mut Runtime, function: usize, args: &[Word]) -> Option<Word> {
    if runtime.loaded.program.functions[function].is_async {
        return Some(runtime.future_new(function, args));
    }
    let code = match crate::jit::compile(runtime, function) {
        Ok(code) => code,
        Err(error) => {
            crate::runtime::fatal(unsafe { &mut *ctx }, runtime, error.0);
            return None;
        }
    };
    // SAFETY: compiled functions take the context and their parameters.
    let result = unsafe { crate::jit::invoke(code, ctx, args) };
    (unsafe { (*ctx).status } == OK).then_some(result)
}

/// Stable ordering of an array: natural order for scalars and text, otherwise
/// a bottom-up merge sort over `compare::Comparing<T>.order`.
fn ordering(ctx: *mut Ctx, runtime: &mut Runtime, function: usize, source: Word) -> Word {
    let program = &runtime.loaded.program;
    let f = &program.functions[function];
    let Some(Type::Array(element)) = program.ty(f.registers[f.params[0].0 as usize]).cloned() else {
        return 0;
    };
    let kind = Kind::of(program, element);
    let mut items = runtime.heap.elements(source).to_vec();
    let element_type = program.ty(element).cloned();
    let builtin = matches!(
        element_type,
        Some(Type::Int | Type::Float | Type::Rat | Type::Byte | Type::Char | Type::Str)
    );
    let repr = kind.repr();
    if builtin {
        items.sort_by(|&a, &b| match element_type {
            Some(Type::Int) => runtime.big_value(a).cmp(&runtime.big_value(b)),
            Some(Type::Float) => f64::from_bits(a).partial_cmp(&f64::from_bits(b)).unwrap_or(Ordering::Equal),
            Some(Type::Rat) => runtime.rational(a).cmp(runtime.rational(b)),
            Some(Type::Str) => runtime.heap.text(a).cmp(runtime.heap.text(b)),
            _ => a.cmp(&b),
        });
        if kind == Kind::Byte {
            let bytes: Vec<u8> = items.iter().map(|&w| w as u8).collect();
            return runtime.heap.allocate_bytes(&bytes);
        }
        return runtime.heap.allocate_array(repr, &items);
    }
    let Some((role, slot, greater)) = comparing(runtime, element) else {
        let ctx = unsafe { &mut *ctx };
        return crate::runtime::fatal(ctx, runtime, "invalid array ordering signature".into());
    };
    // The source keeps every element alive while comparisons run.
    runtime.roots.push((source, Repr::Ref));
    let mut width = 1;
    let mut failed = false;
    while width < items.len() && !failed {
        let mut merged = Vec::with_capacity(items.len());
        let mut left = 0;
        while left < items.len() {
            let mid = (left + width).min(items.len());
            let end = (mid + width).min(items.len());
            let (mut i, mut j) = (left, mid);
            while i < mid && j < end {
                // Take the right element only when the left one is greater.
                match order(ctx, runtime, role, slot, items[i], items[j]) {
                    Some(result) if result == greater => {
                        merged.push(items[j]);
                        j += 1;
                    }
                    Some(_) => {
                        merged.push(items[i]);
                        i += 1;
                    }
                    None => {
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                break;
            }
            merged.extend_from_slice(&items[i..mid]);
            merged.extend_from_slice(&items[j..end]);
            left = end;
        }
        if !failed {
            items = merged;
        }
        width *= 2;
    }
    runtime.roots.pop();
    if failed {
        return 0;
    }
    runtime.heap.allocate_array(repr, &items)
}

/// The `order` method of `compare::Comparing<T>` for element type `element`,
/// and the word of `compare::Order.greater`.
fn comparing(runtime: &Runtime, element: okkama_sparrow::TypeRef) -> Option<(u32, u32, Word)> {
    let program = &runtime.loaded.program;
    program.roles.iter().enumerate().find_map(|(r, decl)| {
        if !decl.name.starts_with("compare::Comparing<") {
            return None;
        }
        let (s, method) = decl
            .methods
            .iter()
            .enumerate()
            .find(|(_, m)| m.name == "order" && m.params == [element])?;
        let Type::Enum(e) = program.ty(method.result?)? else {
            return None;
        };
        let en = program.enums.get(e.0 as usize)?;
        if en.name != "compare::Order" || en.cases != ["less", "equal", "greater"] {
            return None;
        }
        let accepts = match program.ty(element)? {
            Type::Object(o) => program.objects[o.0 as usize].does.iter().any(|w| w.role.0 as usize == r),
            Type::Role(expected) => expected.0 as usize == r,
            _ => false,
        };
        accepts.then_some((r as u32, s as u32, (u64::from(e.0) << 32) | 2))
    })
}

/// `left.order(right)` through the role; None when it failed.
fn order(ctx: *mut Ctx, runtime: &mut Runtime, role: u32, slot: u32, left: Word, right: Word) -> Option<Word> {
    call_role(ctx, runtime, left, role, slot, &[right])
}
