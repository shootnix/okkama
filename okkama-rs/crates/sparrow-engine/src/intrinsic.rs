//! Verified intrinsic entrypoints lower to small VM plans. State and notifications
//! belong to this engine and Tokio, never to the legacy intrinsic dispatcher.
use crate::{
    Error, FunctionPlan, Op, Program, Type, liveness,
    value::{Heap, Slot},
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    FakeStorage(crate::fake_storage::Operation),
    ScalarOrdering,
    Test(crate::testing::Operation),
    Stream(crate::stream::Operation),
    Barrier(crate::barrier::Operation),
    Collect(crate::collect::Operation),
    Group(crate::group::Operation),
    Channel(crate::channel::Operation),
    EventNew(usize),
    EventReady,
    EventSet,
    EventWait,
    SemaphoreAvailable,
    SemaphoreAcquire(usize),
    LockNew(usize),
    LockLocked,
    LockAcquire(usize),
    GrantClosed,
    GrantClose,
    SignalNew(usize),
    SignalToken(usize),
    SignalRequested,
    SignalCancel,
    SignalWait,
    UnattachedToken(usize),
    AtomicNew(usize),
    AtomicValue,
    AtomicReplace,
    AtomicCompareReplace,
    AtomicClosed,
    AtomicClose,
}

pub(crate) fn lower(program: &Program, index: usize) -> Result<FunctionPlan, Error> {
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("intrinsic function");
    if intrinsic.name.starts_with("okkama.fake.storage.") {
        return crate::fake_storage::lower(program, index);
    }
    if intrinsic.name == "okkama.array.ordering" {
        return crate::ordering::lower(program, index);
    }
    if intrinsic.name.starts_with("okkama.test.") {
        return crate::testing::lower(program, index);
    }
    if intrinsic
        .name
        .starts_with("okkama.collect.FutureCompletions.")
    {
        return crate::stream::lower(program, index);
    }
    if intrinsic.name.starts_with("okkama.timeout.") {
        return crate::timeout::lower(program, index);
    }
    if intrinsic.name.starts_with("okkama.collect.") {
        return crate::collect::lower(program, index);
    }
    if intrinsic.name == "okkama.scope.FutureScope.result" {
        return crate::group::lower_scope(program, index);
    }
    if intrinsic.name.starts_with("okkama.scope.FutureGroup.") {
        return crate::group::lower(program, index);
    }
    if intrinsic.name.starts_with("okkama.channel.") {
        return crate::channel::lower(program, index);
    }
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    let params: Vec<_> = f.params.iter().map(|r| r.0 as usize).collect();
    let param_types: Vec<_> = params.iter().map(|&r| program.ty(f.registers[r])).collect();
    let result = f.result.and_then(|t| program.ty(t));
    let event_object = |ty: Option<&Type>| -> Result<usize, Error> {
        let Some(Type::Object(owner)) = ty else {
            return Err(invalid());
        };
        let object = &program.objects[owner.0 as usize];
        if object.name != "async.event::Event"
            || object.destroy.is_some()
            || object.fields.len() != 1
            || object.fields[0].0 != "initial"
            || program.ty(object.fields[0].1) != Some(&Type::Bool)
        {
            return Err(invalid());
        }
        Ok(owner.0 as usize)
    };
    let plain_object = |ty: Option<&Type>, name: &str| -> Result<usize, Error> {
        let Some(Type::Object(owner)) = ty else {
            return Err(invalid());
        };
        let object = &program.objects[owner.0 as usize];
        if object.name != name || object.destroy.is_some() || !object.fields.is_empty() {
            return Err(invalid());
        }
        Ok(owner.0 as usize)
    };
    let cancelling_result = |owner: usize| -> Result<(), Error> {
        let Some(Type::Role(role)) = result else {
            return Err(invalid());
        };
        if program.roles[role.0 as usize].name != "async.cancellation::Cancelling"
            || !program.objects[owner].does.iter().any(|w| w.role == *role)
        {
            return Err(invalid());
        }
        Ok(())
    };
    if !intrinsic.helpers.is_empty() {
        return Err(invalid());
    }
    let (operation, argument, is_async) = match intrinsic.name.as_str() {
        name if name.starts_with("okkama.atomic.Atomic.") => {
            let new = name.ends_with("new_value");
            let receiver_type = if new {
                result
            } else {
                param_types.first().copied().flatten()
            };
            let Some(Type::Object(owner)) = receiver_type else {
                return Err(invalid());
            };
            let owner = owner.0 as usize;
            let object = &program.objects[owner];
            if !object.name.starts_with("async.atomic::Atomic<")
                || object.destroy.is_some()
                || !object.fields.is_empty()
            {
                return Err(invalid());
            }
            // close/closed do not inspect T. A reachable disposal witness may
            // survive compiler pruning even when its constructor is absent.
            let value_type = if matches!(
                name,
                "okkama.atomic.Atomic.close" | "okkama.atomic.Atomic.closed"
            ) {
                None
            } else {
                let ctor = program
                    .functions
                    .iter()
                    .find(|f| {
                        f.intrinsic.as_ref().is_some_and(|i| {
                            i.name == "okkama.atomic.Atomic.new_value"
                                && i.objects.len() == 1
                                && i.objects[0].0 as usize == owner
                        })
                    })
                    .ok_or_else(invalid)?;
                if ctor.params.len() != 1 {
                    return Err(invalid());
                }
                program.ty(ctor.registers[ctor.params[0].0 as usize])
            };
            let operation = match name {
                "okkama.atomic.Atomic.new_value" if param_types == vec![value_type] => {
                    Operation::AtomicNew(owner)
                }
                "okkama.atomic.Atomic.value" if params.len() == 1 && result == value_type => {
                    Operation::AtomicValue
                }
                "okkama.atomic.Atomic.replace"
                    if param_types == vec![receiver_type, value_type] && result.is_none() =>
                {
                    Operation::AtomicReplace
                }
                "okkama.atomic.Atomic.compare_and_replace"
                    if param_types == vec![receiver_type, value_type, value_type]
                        && result == Some(&Type::Bool) =>
                {
                    Operation::AtomicCompareReplace
                }
                "okkama.atomic.Atomic.closed"
                    if params.len() == 1 && result == Some(&Type::Bool) =>
                {
                    Operation::AtomicClosed
                }
                "okkama.atomic.Atomic.close" if params.len() == 1 && result.is_none() => {
                    Operation::AtomicClose
                }
                _ => return Err(invalid()),
            };
            if new {
                if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner {
                    return Err(invalid());
                }
            } else if matches!(operation, Operation::AtomicClosed | Operation::AtomicClose) {
                if !intrinsic.objects.is_empty() {
                    return Err(invalid());
                }
            } else {
                if intrinsic.objects.len() != 1 {
                    return Err(invalid());
                }
                let closed = &program.objects[intrinsic.objects[0].0 as usize];
                if closed.name != "async.atomic::AtomicClosed"
                    || !closed.fields.is_empty()
                    || closed.destroy.is_some()
                {
                    return Err(invalid());
                }
            }
            (operation, params.first().copied(), false)
        }
        "okkama.cancellation.Cancellation.new" | "okkama.cancellation.CancellationToken.new" => {
            if !params.is_empty() {
                return Err(invalid());
            }
            let token = intrinsic.name.contains("CancellationToken");
            let owner = plain_object(
                result,
                if token {
                    "async.cancellation::CancellationToken"
                } else {
                    "async.cancellation::Cancellation"
                },
            )?;
            if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner {
                return Err(invalid());
            }
            (
                if token {
                    Operation::UnattachedToken(owner)
                } else {
                    Operation::SignalNew(owner)
                },
                None,
                false,
            )
        }
        "okkama.cancellation.Cancellation.cancelling" => {
            if params.len() != 1 || intrinsic.objects.len() != 1 {
                return Err(invalid());
            }
            plain_object(param_types[0], "async.cancellation::Cancellation")?;
            let owner = intrinsic.objects[0].0 as usize;
            let object = &program.objects[owner];
            if object.name != "async.cancellation::CancellationToken"
                || !object.fields.is_empty()
                || object.destroy.is_some()
            {
                return Err(invalid());
            }
            cancelling_result(owner)?;
            (Operation::SignalToken(owner), Some(params[0]), false)
        }
        "okkama.cancellation.Cancellation.cancel"
        | "okkama.cancellation.CancellationToken.requested"
        | "okkama.cancellation.CancellationToken.check"
        | "okkama.cancellation.CancellationToken.wait" => {
            if params.len() != 1 || !intrinsic.objects.is_empty() {
                return Err(invalid());
            }
            let cancel = intrinsic.name.ends_with(".cancel");
            let owner = plain_object(
                param_types[0],
                if cancel {
                    "async.cancellation::Cancellation"
                } else {
                    "async.cancellation::CancellationToken"
                },
            )?;
            let operation = match intrinsic.name.as_str() {
                "okkama.cancellation.Cancellation.cancel" if result.is_none() => {
                    Operation::SignalCancel
                }
                "okkama.cancellation.CancellationToken.requested"
                    if result == Some(&Type::Bool) =>
                {
                    Operation::SignalRequested
                }
                "okkama.cancellation.CancellationToken.check" if result.is_none() => {
                    Operation::SignalRequested
                }
                "okkama.cancellation.CancellationToken.wait" => {
                    cancelling_result(owner)?;
                    Operation::SignalWait
                }
                _ => return Err(invalid()),
            };
            (
                operation,
                Some(params[0]),
                matches!(operation, Operation::SignalWait),
            )
        }
        name if name.starts_with("okkama.barrier.") => {
            if params.len() != 1 {
                return Err(invalid());
            }
            let operation = if name.starts_with("okkama.barrier.BarrierState.") {
                plain_object(param_types[0], "async.barrier::BarrierState")?;
                if !intrinsic.objects.is_empty() {
                    return Err(invalid());
                }
                let field = match name {
                    "okkama.barrier.BarrierState.parties" => 0,
                    "okkama.barrier.BarrierState.waiters" => 1,
                    "okkama.barrier.BarrierState.generation" => 2,
                    "okkama.barrier.BarrierState.broken" => 3,
                    _ => return Err(invalid()),
                };
                if result != Some(if field == 3 { &Type::Bool } else { &Type::Int }) {
                    return Err(invalid());
                }
                crate::barrier::Operation::Read(field)
            } else {
                let Some(Type::Object(owner)) = param_types[0] else {
                    return Err(invalid());
                };
                let object = &program.objects[owner.0 as usize];
                if object.name != "async.barrier::Barrier"
                    || object.destroy.is_some()
                    || object.fields.len() != 1
                    || object.fields[0].0 != "amount"
                    || program.ty(object.fields[0].1) != Some(&Type::Int)
                {
                    return Err(invalid());
                }
                match name {
                    "okkama.barrier.Barrier.wait" => {
                        if result != Some(&Type::Int) || intrinsic.objects.len() != 1 {
                            return Err(invalid());
                        }
                        let owner = intrinsic.objects[0].0;
                        let object = &program.objects[owner as usize];
                        if object.name != "async.barrier::BrokenBarrier"
                            || !object.fields.is_empty()
                            || object.destroy.is_some()
                        {
                            return Err(invalid());
                        }
                        crate::barrier::Operation::Wait(owner)
                    }
                    "okkama.barrier.Barrier.state" => {
                        let owner = plain_object(result, "async.barrier::BarrierState")?;
                        if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner
                        {
                            return Err(invalid());
                        }
                        crate::barrier::Operation::Snapshot(owner as u32)
                    }
                    "okkama.barrier.Barrier.abort" | "okkama.barrier.Barrier.reset" => {
                        if result.is_some() || !intrinsic.objects.is_empty() {
                            return Err(invalid());
                        }
                        if name.ends_with(".abort") {
                            crate::barrier::Operation::Abort
                        } else {
                            crate::barrier::Operation::Reset
                        }
                    }
                    _ => return Err(invalid()),
                }
            };
            (
                Operation::Barrier(operation),
                Some(params[0]),
                matches!(operation, crate::barrier::Operation::Wait(_)),
            )
        }
        "okkama.semaphore.Semaphore.permits" | "okkama.semaphore.Semaphore.permit" => {
            if params.len() != 1 {
                return Err(invalid());
            }
            let Some(Type::Object(owner)) = param_types[0] else {
                return Err(invalid());
            };
            let object = &program.objects[owner.0 as usize];
            if object.name != "async.semaphore::Semaphore"
                || object.destroy.is_some()
                || object.fields.len() != 1
                || object.fields[0].0 != "amount"
                || program.ty(object.fields[0].1) != Some(&Type::Int)
            {
                return Err(invalid());
            }
            if intrinsic.name.ends_with(".permits") {
                if result != Some(&Type::Int) || !intrinsic.objects.is_empty() {
                    return Err(invalid());
                }
                (Operation::SemaphoreAvailable, Some(params[0]), false)
            } else {
                let owner = plain_object(result, "async.semaphore::Permit")?;
                if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner {
                    return Err(invalid());
                }
                (Operation::SemaphoreAcquire(owner), Some(params[0]), true)
            }
        }
        "okkama.semaphore.Permit.closed" | "okkama.semaphore.Permit.close" => {
            if params.len() != 1 || !intrinsic.objects.is_empty() {
                return Err(invalid());
            }
            plain_object(param_types[0], "async.semaphore::Permit")?;
            let closed = intrinsic.name.ends_with(".closed");
            if if closed {
                result != Some(&Type::Bool)
            } else {
                result.is_some()
            } {
                return Err(invalid());
            }
            (
                if closed {
                    Operation::GrantClosed
                } else {
                    Operation::GrantClose
                },
                Some(params[0]),
                false,
            )
        }
        "okkama.lock.Lock.new" => {
            if !params.is_empty() {
                return Err(invalid());
            }
            let owner = plain_object(result, "async.lock::Lock")?;
            if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner {
                return Err(invalid());
            }
            (Operation::LockNew(owner), None, false)
        }
        "okkama.lock.Lock.guard" => {
            if params.len() != 1 {
                return Err(invalid());
            }
            plain_object(param_types[0], "async.lock::Lock")?;
            let owner = plain_object(result, "async.lock::LockGuard")?;
            if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner {
                return Err(invalid());
            }
            (Operation::LockAcquire(owner), Some(params[0]), true)
        }
        "okkama.lock.Lock.locked"
        | "okkama.lock.LockGuard.closed"
        | "okkama.lock.LockGuard.close" => {
            if params.len() != 1 || !intrinsic.objects.is_empty() {
                return Err(invalid());
            }
            let locked = intrinsic.name.ends_with(".locked");
            plain_object(
                param_types[0],
                if locked {
                    "async.lock::Lock"
                } else {
                    "async.lock::LockGuard"
                },
            )?;
            let op = match intrinsic.name.as_str() {
                "okkama.lock.Lock.locked" if result == Some(&Type::Bool) => Operation::LockLocked,
                "okkama.lock.LockGuard.closed" if result == Some(&Type::Bool) => {
                    Operation::GrantClosed
                }
                "okkama.lock.LockGuard.close" if result.is_none() => Operation::GrantClose,
                _ => return Err(invalid()),
            };
            (op, Some(params[0]), false)
        }
        "okkama.event.Event.new" | "okkama.event.Event.new_ready" => {
            let ready = intrinsic.name.ends_with("new_ready");
            if param_types
                != if ready {
                    vec![Some(&Type::Bool)]
                } else {
                    vec![]
                }
            {
                return Err(invalid());
            }
            let owner = event_object(result)?;
            if intrinsic.objects.len() != 1 || intrinsic.objects[0].0 as usize != owner {
                return Err(invalid());
            }
            (Operation::EventNew(owner), params.first().copied(), false)
        }
        "okkama.event.Event.ready" | "okkama.event.Event.set" | "okkama.event.Event.wait" => {
            if params.len() != 1 || !intrinsic.objects.is_empty() {
                return Err(invalid());
            }
            event_object(param_types[0])?;
            let op = match intrinsic.name.as_str() {
                "okkama.event.Event.ready" if result == Some(&Type::Bool) => Operation::EventReady,
                "okkama.event.Event.set" if result.is_none() => Operation::EventSet,
                "okkama.event.Event.wait" if result == param_types[0] => Operation::EventWait,
                _ => return Err(invalid()),
            };
            (op, Some(params[0]), matches!(op, Operation::EventWait))
        }
        _ => {
            return Err(Error(format!(
                "unsupported intrinsic {} in new engine",
                intrinsic.name
            )));
        }
    };
    if f.is_async != is_async {
        return Err(invalid());
    }
    let output = f.registers.len();
    let args = if intrinsic.name.starts_with("okkama.atomic.") {
        std::array::from_fn(|i| params.get(i).copied())
    } else {
        [argument, None, None]
    };
    let mut plan = vec![Op::Intrinsic(operation, output, args)];
    let registers = if matches!(
        operation,
        Operation::AtomicValue | Operation::AtomicReplace | Operation::AtomicCompareReplace
    ) {
        plan = vec![
            Op::Intrinsic(
                Operation::AtomicClosed,
                output,
                [params.first().copied(), None, None],
            ),
            Op::Branch(
                output,
                okkama_sparrow::Target {
                    pc: 4,
                    moves: vec![],
                },
                okkama_sparrow::Target {
                    pc: 2,
                    moves: vec![],
                },
            ),
            Op::Intrinsic(operation, output, args),
            if result.is_some() {
                Op::Return(output)
            } else {
                Op::ReturnVoid
            },
            Op::ObjectNew(output, intrinsic.objects[0].0, vec![]),
            Op::Throw(output),
        ];
        output + 1
    } else if intrinsic.name == "okkama.cancellation.CancellationToken.check" {
        plan.push(Op::Branch(
            output,
            okkama_sparrow::Target {
                pc: 3,
                moves: vec![],
            },
            okkama_sparrow::Target {
                pc: 2,
                moves: vec![],
            },
        ));
        plan.push(Op::ReturnVoid);
        plan.push(Op::Const(
            output,
            Slot {
                tag: crate::value::CANCELLED,
                bits: 0,
            },
        ));
        plan.push(Op::Throw(output));
        output + 1
    } else if is_async {
        plan.push(Op::Await {
            dst: Some(output + 1),
            future: output,
        });
        plan.push(Op::Return(output + 1));
        output + 2
    } else {
        plan.push(if result.is_some() {
            Op::Return(output)
        } else {
            Op::ReturnVoid
        });
        output + 1
    };
    let live = liveness::analyze(&plan, &[]);
    let call_saved = liveness::call_saved(&plan, &live, &[]);
    Ok(FunctionPlan {
        native: None,
        is_async,
        cold_entry: 0,
        cached: false,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params,
        plan,
        registers,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers: vec![],
    })
}

impl Heap {
    pub(crate) fn intrinsic(
        &mut self,
        operation: Operation,
        argument: Option<Slot>,
    ) -> Result<Slot, Error> {
        if let Operation::ScalarOrdering = operation {
            return self
                .scalar_ordering(argument.ok_or_else(|| Error("missing ordering array".into()))?);
        }
        if let Operation::UnattachedToken(owner) = operation {
            return self.object_new(owner as u32, std::iter::empty());
        }
        if let Operation::SignalNew(owner) = operation {
            let object = self.object_new(owner as u32, std::iter::empty())?;
            self.events
                .insert(object.bits, tokio::sync::watch::channel(false).0);
            return Ok(object);
        }
        if let Operation::SignalToken(owner) = operation {
            let receiver = argument.ok_or_else(|| Error("missing cancellation receiver".into()))?;
            let signal = self
                .events
                .get(&receiver.bits)
                .ok_or_else(|| Error("Invalid cancellation capability".into()))?
                .clone();
            let token = self.object_new(owner as u32, std::iter::empty())?;
            self.events.insert(token.bits, signal);
            return Ok(token);
        }
        if matches!(
            operation,
            Operation::LockNew(_)
                | Operation::SemaphoreAvailable
                | Operation::SemaphoreAcquire(_)
                | Operation::LockLocked
                | Operation::LockAcquire(_)
                | Operation::GrantClosed
                | Operation::GrantClose
        ) {
            return self.lock_operation(operation, argument);
        }
        if let Operation::EventNew(owner) = operation {
            let ready = argument.unwrap_or_else(|| Slot::boolean(false));
            let object = self.object_new(owner as u32, [ready].into_iter())?;
            self.events
                .insert(object.bits, tokio::sync::watch::channel(ready.bits != 0).0);
            return Ok(object);
        }
        let receiver = argument.ok_or_else(|| Error("missing event receiver".into()))?;
        self.object_descriptor(receiver)?;
        let event = self.events.get(&receiver.bits).ok_or_else(|| {
            Error(
                if matches!(
                    operation,
                    Operation::SignalRequested | Operation::SignalCancel | Operation::SignalWait
                ) {
                    "Invalid cancellation capability"
                } else {
                    "Invalid async event"
                }
                .into(),
            )
        })?;
        let ready = *event.borrow();
        match operation {
            Operation::EventReady | Operation::SignalRequested => Ok(Slot::boolean(ready)),
            Operation::EventSet | Operation::SignalCancel => {
                if !ready {
                    event.send_replace(true);
                }
                Ok(Slot::default())
            }
            Operation::EventWait | Operation::SignalWait => {
                let state = if ready {
                    crate::future::State::Ready(Ok(receiver))
                } else {
                    crate::future::State::Event(receiver)
                };
                let future = self.future_with_state(state)?;
                self.futures.get_mut(future.bits)?.captures = vec![receiver].into_boxed_slice();
                Ok(future)
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_disposal_witness_does_not_require_an_unreachable_constructor() {
        let program = okkama_sparrow::assemble(
            r#"sparrow 9
module "test" "v1.0.0"
type t0 object obj0
type t1 bool
object obj0 "async.atomic::Atomic<int>" {}
function f0 "close" intrinsic "okkama.atomic.Atomic.close" [] [] {
 registers [t0]
 params [r0]
 result void
}
function f1 "closed" intrinsic "okkama.atomic.Atomic.closed" [] [] {
 registers [t0]
 params [r0]
 result t1
}
"#,
        )
        .unwrap();
        for index in 0..2 {
            assert!(lower(&program, index).is_ok());
        }
    }
    #[test]
    fn pending_events_are_gc_roots_and_cancel_does_not_signal_the_event() {
        let mut heap = Heap::new(&[], &[]);
        let event = heap.intrinsic(Operation::EventNew(0), None).unwrap();
        let waiting = heap.intrinsic(Operation::EventWait, Some(event)).unwrap();
        for _ in 0..20 {
            for _ in 0..100 {
                heap.intrinsic(Operation::EventNew(0), None).unwrap();
            }
            heap.collect([waiting].into_iter()).unwrap();
            assert_eq!(heap.events.len(), 1);
            assert_eq!(
                heap.intrinsic(Operation::EventReady, Some(event))
                    .unwrap()
                    .bits,
                0
            );
        }
        heap.future_control(okkama_sparrow::FutOp::Cancel, waiting)
            .unwrap();
        assert_eq!(
            heap.intrinsic(Operation::EventReady, Some(event))
                .unwrap()
                .bits,
            0
        );
        heap.intrinsic(Operation::EventSet, Some(event)).unwrap();
        assert_eq!(
            heap.intrinsic(Operation::EventReady, Some(event))
                .unwrap()
                .bits,
            1
        );
        assert!(matches!(
            heap.future(waiting).unwrap().state,
            crate::future::State::Ready(Err(_))
        ));
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.events.is_empty());
        assert!(heap.future(waiting).is_err());
    }
}

/// Okkama guards release only through close(). GC must not silently unlock.
#[derive(Debug)]
pub(crate) struct Grant {
    native: Option<tokio::sync::OwnedSemaphorePermit>,
    pool: Option<std::rc::Rc<crate::semaphore::Pool>>,
    closed: bool,
}
impl Grant {
    pub(crate) fn semaphore(pool: std::rc::Rc<crate::semaphore::Pool>) -> Self {
        Self {
            native: None,
            pool: Some(pool),
            closed: false,
        }
    }
    fn close(&mut self) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        if let Some(pool) = self.pool.take() {
            pool.release();
        }
        drop(self.native.take());
    }
}
impl Drop for Grant {
    fn drop(&mut self) {
        if let Some(permit) = self.native.take() {
            permit.forget();
        }
    }
}
impl Heap {
    pub(crate) fn grant(
        &mut self,
        owner: u32,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<Slot, Error> {
        let guard = self.object_new(owner, std::iter::empty())?;
        self.grants.insert(
            guard.bits,
            Grant {
                native: Some(permit),
                pool: None,
                closed: false,
            },
        );
        Ok(guard)
    }
    fn lock_operation(
        &mut self,
        operation: Operation,
        argument: Option<Slot>,
    ) -> Result<Slot, Error> {
        if let Operation::LockNew(owner) = operation {
            let lock = self.object_new(owner as u32, std::iter::empty())?;
            self.locks.insert(
                lock.bits,
                std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            );
            return Ok(lock);
        }
        let receiver = argument.ok_or_else(|| Error("missing lock receiver".into()))?;
        self.object_descriptor(receiver)?;
        match operation {
            Operation::GrantClosed => Ok(Slot::boolean(
                self.grants
                    .get(&receiver.bits)
                    .ok_or_else(|| Error("Invalid async lock guard".into()))?
                    .closed,
            )),
            Operation::GrantClose => {
                self.grants
                    .get_mut(&receiver.bits)
                    .ok_or_else(|| Error("Invalid async lock guard".into()))?
                    .close();
                Ok(Slot::default())
            }
            Operation::SemaphoreAvailable => {
                Ok(Slot::small(self.semaphore_pool(receiver)?.available()))
            }
            Operation::SemaphoreAcquire(owner) => {
                let pool = self.semaphore_pool(receiver)?;
                let state = match pool.gate.clone().try_acquire_owned() {
                    Ok(permit) => crate::future::State::Ready(Ok(self.semaphore_grant(
                        owner as u32,
                        pool.clone(),
                        permit,
                    )?)),
                    Err(tokio::sync::TryAcquireError::NoPermits) => {
                        crate::future::State::Permit(receiver, owner as u32)
                    }
                    Err(_) => return Err(Error("native semaphore unexpectedly closed".into())),
                };
                let future = self.future_with_state(state)?;
                self.futures.get_mut(future.bits)?.captures = vec![receiver].into_boxed_slice();
                if matches!(state, crate::future::State::Permit(_, _)) {
                    self.futures.get_mut(future.bits)?.permit =
                        Some(PermitRequest::for_pool(pool)?);
                }
                Ok(future)
            }
            Operation::LockLocked => Ok(Slot::boolean(
                self.locks
                    .get(&receiver.bits)
                    .ok_or_else(|| Error("Invalid async lock".into()))?
                    .available_permits()
                    == 0,
            )),
            Operation::LockAcquire(owner) => {
                let lock = self
                    .locks
                    .get(&receiver.bits)
                    .ok_or_else(|| Error("Invalid async lock".into()))?
                    .clone();
                let state = match lock.clone().try_acquire_owned() {
                    Ok(permit) => {
                        crate::future::State::Ready(Ok(self.grant(owner as u32, permit)?))
                    }
                    Err(tokio::sync::TryAcquireError::NoPermits) => {
                        crate::future::State::Permit(receiver, owner as u32)
                    }
                    Err(tokio::sync::TryAcquireError::Closed) => {
                        return Err(Error("native lock unexpectedly closed".into()));
                    }
                };
                let future = self.future_with_state(state)?;
                self.futures.get_mut(future.bits)?.captures = vec![receiver].into_boxed_slice();
                if matches!(state, crate::future::State::Permit(_, _)) {
                    self.futures.get_mut(future.bits)?.permit = Some(PermitRequest::new(lock)?);
                }
                Ok(future)
            }
            _ => Err(Error("invalid lock operation".into())),
        }
    }
}

/// The native acquisition stays alive between host polls. Its queue position is
/// established immediately, and language cancellation can remove it immediately.
type Acquisition = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::AcquireError>,
            >,
    >,
>;
pub(crate) struct PermitRequest(
    std::cell::RefCell<Option<Acquisition>>,
    Option<std::rc::Rc<crate::semaphore::Pool>>,
    std::cell::Cell<bool>,
);
impl std::fmt::Debug for PermitRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PermitRequest")
    }
}
impl PermitRequest {
    fn new(lock: std::sync::Arc<tokio::sync::Semaphore>) -> Result<std::rc::Rc<Self>, Error> {
        let mut acquire: Acquisition = Box::pin(lock.acquire_owned());
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        if acquire.as_mut().poll(&mut cx).is_ready() {
            return Err(Error(
                "lock acquisition unexpectedly ready after failed try_acquire".into(),
            ));
        }
        Ok(std::rc::Rc::new(Self(
            std::cell::RefCell::new(Some(acquire)),
            None,
            std::cell::Cell::new(false),
        )))
    }
    fn for_pool(pool: std::rc::Rc<crate::semaphore::Pool>) -> Result<std::rc::Rc<Self>, Error> {
        let mut request = Self::new(pool.gate.clone())?;
        let unique = std::rc::Rc::get_mut(&mut request).expect("new request");
        pool.pending.set(pool.pending.get() + 1);
        unique.1 = Some(pool);
        unique.2.set(true);
        Ok(request)
    }
    fn unregister(&self) {
        if self.2.replace(false) {
            let pool = self.1.as_ref().expect("pool registration");
            pool.pending.set(pool.pending.get() - 1);
        }
    }
    pub fn poll(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<tokio::sync::OwnedSemaphorePermit, Error>> {
        let mut state = self.0.borrow_mut();
        let Some(acquire) = state.as_mut() else {
            return std::task::Poll::Pending;
        };
        let result = acquire
            .as_mut()
            .poll(cx)
            .map_err(|_| Error("native lock unexpectedly closed".into()));
        if result.is_ready() {
            self.unregister();
            state.take();
        }
        result
    }
    pub fn cancel(&self) {
        self.unregister();
        self.0.borrow_mut().take();
    }
}

impl Drop for PermitRequest {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;
    fn granted(heap: &Heap, future: Slot) -> Slot {
        let crate::future::State::Ready(Ok(guard)) = heap.future(future).unwrap().state else {
            panic!("missing grant");
        };
        guard
    }
    #[test]
    fn gc_does_not_implicitly_close_a_guard_but_explicit_close_is_idempotent() {
        let mut heap = Heap::new(&[], &[]);
        let lock = heap.intrinsic(Operation::LockNew(0), None).unwrap();
        let ready = heap
            .intrinsic(Operation::LockAcquire(1), Some(lock))
            .unwrap();
        let guard = granted(&heap, ready);
        assert_eq!(
            heap.intrinsic(Operation::LockLocked, Some(lock))
                .unwrap()
                .bits,
            1
        );
        heap.intrinsic(Operation::GrantClose, Some(guard)).unwrap();
        heap.intrinsic(Operation::GrantClose, Some(guard)).unwrap();
        assert_eq!(
            heap.intrinsic(Operation::GrantClosed, Some(guard))
                .unwrap()
                .bits,
            1
        );
        assert_eq!(
            heap.intrinsic(Operation::LockLocked, Some(lock))
                .unwrap()
                .bits,
            0
        );
        heap.intrinsic(Operation::LockAcquire(1), Some(lock))
            .unwrap();
        heap.collect([lock].into_iter()).unwrap();
        assert!(heap.grants.is_empty());
        assert_eq!(
            heap.intrinsic(Operation::LockLocked, Some(lock))
                .unwrap()
                .bits,
            1
        );
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.locks.is_empty());
    }
    #[test]
    fn native_queue_is_registered_before_host_poll_and_cancel_removes_it_immediately() {
        let mut heap = Heap::new(&[], &[]);
        let lock = heap.intrinsic(Operation::LockNew(0), None).unwrap();
        let ready = heap
            .intrinsic(Operation::LockAcquire(1), Some(lock))
            .unwrap();
        let guard = granted(&heap, ready);
        let cancelled = heap
            .intrinsic(Operation::LockAcquire(1), Some(lock))
            .unwrap();
        let next = heap
            .intrinsic(Operation::LockAcquire(1), Some(lock))
            .unwrap();
        heap.future_control(okkama_sparrow::FutOp::Cancel, cancelled)
            .unwrap();
        heap.intrinsic(Operation::GrantClose, Some(guard)).unwrap();
        let late = heap
            .intrinsic(Operation::LockAcquire(1), Some(lock))
            .unwrap();
        assert!(matches!(
            heap.future(late).unwrap().state,
            crate::future::State::Permit(_, _)
        ));
        let request = heap.future(next).unwrap().permit.as_ref().unwrap().clone();
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let std::task::Poll::Ready(Ok(permit)) = request.poll(&mut cx) else {
            panic!("FIFO successor did not receive grant");
        };
        let next_guard = heap.grant(1, permit).unwrap();
        heap.future_state(next, crate::future::State::Ready(Ok(next_guard)))
            .unwrap();
        heap.intrinsic(Operation::GrantClose, Some(next_guard))
            .unwrap();
        assert!(
            heap.future(late)
                .unwrap()
                .permit
                .as_ref()
                .unwrap()
                .poll(&mut cx)
                .is_ready()
        );
    }
}

impl Heap {
    pub(crate) fn atomic_operation(
        &mut self,
        operation: Operation,
        args: [Option<Slot>; 3],
    ) -> Result<Slot, Error> {
        let first = args[0].ok_or_else(|| Error("missing atomic argument".into()))?;
        if let Operation::AtomicNew(owner) = operation {
            let object = self.object_new(owner as u32, std::iter::empty())?;
            self.atomics.insert(object.bits, Some(first));
            return Ok(object);
        }
        self.object_descriptor(first)?;
        let value = *self
            .atomics
            .get(&first.bits)
            .ok_or_else(|| Error("Invalid atomic state".into()))?;
        match operation {
            Operation::AtomicClosed => Ok(Slot::boolean(value.is_none())),
            Operation::AtomicClose => {
                self.atomics.insert(first.bits, None);
                Ok(Slot::default())
            }
            Operation::AtomicValue => {
                value.ok_or_else(|| Error("closed atomic bypassed intrinsic guard".into()))
            }
            Operation::AtomicReplace => {
                self.atomics.insert(first.bits, args[1]);
                Ok(Slot::default())
            }
            Operation::AtomicCompareReplace => {
                let equal = self.equal(
                    value.ok_or_else(|| Error("closed atomic bypassed intrinsic guard".into()))?,
                    args[1].ok_or_else(|| Error("missing CAS expected value".into()))?,
                )?;
                if equal {
                    self.atomics.insert(first.bits, args[2]);
                }
                Ok(Slot::boolean(equal))
            }
            _ => Err(Error("invalid atomic operation".into())),
        }
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;
    #[test]
    fn native_cell_traces_its_value_and_close_releases_it_without_resource_ownership() {
        let mut heap = Heap::new(&[], &[]);
        let text = heap.allocate_text("retained".into()).unwrap();
        let cell = heap
            .atomic_operation(Operation::AtomicNew(0), [Some(text), None, None])
            .unwrap();
        heap.collect([cell].into_iter()).unwrap();
        assert_eq!(heap.text(text).unwrap(), "retained");
        let replaced = heap.allocate_text("replaced".into()).unwrap();
        heap.atomic_operation(Operation::AtomicReplace, [Some(cell), Some(replaced), None])
            .unwrap();
        heap.collect([cell].into_iter()).unwrap();
        assert!(heap.text(text).is_err());
        assert_eq!(heap.text(replaced).unwrap(), "replaced");
        heap.atomic_operation(Operation::AtomicClose, [Some(cell), None, None])
            .unwrap();
        heap.collect([cell].into_iter()).unwrap();
        assert!(heap.text(replaced).is_err());
        assert_eq!(
            heap.atomic_operation(Operation::AtomicClosed, [Some(cell), None, None])
                .unwrap()
                .bits,
            1
        );
        heap.collect(std::iter::empty()).unwrap();
        assert!(heap.atomics.is_empty());
    }
    #[test]
    fn failed_cas_comparison_does_not_publish_a_new_value() {
        let mut heap = Heap::new(&[], &[]);
        let original = heap.any_box(Slot::small(1)).unwrap();
        let other = heap.any_box(Slot::small(1)).unwrap();
        let revised = heap.any_box(Slot::small(2)).unwrap();
        let cell = heap
            .atomic_operation(Operation::AtomicNew(0), [Some(original), None, None])
            .unwrap();
        assert!(
            heap.atomic_operation(
                Operation::AtomicCompareReplace,
                [Some(cell), Some(other), Some(revised)]
            )
            .is_err()
        );
        assert_eq!(
            heap.atomic_operation(Operation::AtomicValue, [Some(cell), None, None])
                .unwrap()
                .bits,
            original.bits
        );
        assert_eq!(
            heap.atomic_operation(
                Operation::AtomicCompareReplace,
                [Some(cell), Some(original), Some(revised)]
            )
            .unwrap()
            .bits,
            1
        );
        assert_eq!(
            heap.atomic_operation(Operation::AtomicValue, [Some(cell), None, None])
                .unwrap()
                .bits,
            revised.bits
        );
    }
}
