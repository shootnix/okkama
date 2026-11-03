//! Structured groups reuse ordinary VM continuations and Tokio-spawned tasks.
//! There is no second executor: wait is a generated loop over registered futures.
use crate::{
    Error,
    failure::FailureKind,
    future::State,
    value::{FAILURE, Heap, Slot},
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    New(u32),
    Allowed,
    Register,
    Active,
    Cancel,
    Entries,
    WaitFailed,
    PreserveFailure,
    Finish,
}
#[derive(Debug, Default)]
pub(crate) struct Group {
    sealed: bool,
    cancelled: bool,
    entries: Vec<Slot>,
    cancellation_owned: Vec<bool>,
    result: Option<Result<Slot, Slot>>,
}
impl Group {
    pub(crate) fn roots(&self, roots: &mut Vec<Slot>) {
        roots.extend_from_slice(&self.entries);
        if let Some(result) = self.result {
            roots.push(match result {
                Ok(v) | Err(v) => v,
            });
        }
    }
}
impl Heap {
    fn group_cancel(&mut self, receiver: Slot) -> Result<(), Error> {
        let group = self
            .groups
            .get_mut(&receiver.bits)
            .ok_or_else(|| Error("Invalid future group".into()))?;
        group.sealed = true;
        if std::mem::replace(&mut group.cancelled, true) {
            return Ok(());
        }
        let entries = group.entries.clone();
        for (index, handle) in entries.into_iter().enumerate() {
            let owned = match self.future(handle)?.state {
                State::Ready(Err(f)) => matches!(self.failure(f)?.kind, FailureKind::Cancelled),
                State::Ready(Ok(_)) => false,
                _ => true,
            };
            self.groups
                .get_mut(&receiver.bits)
                .expect("group")
                .cancellation_owned[index] = owned;
            if owned {
                self.future_control(okkama_sparrow::FutOp::Cancel, handle)?;
            }
        }
        Ok(())
    }
    pub(crate) fn group_operation(
        &mut self,
        op: Operation,
        args: [Option<Slot>; 3],
    ) -> Result<Slot, Error> {
        if let Operation::New(owner) = op {
            let value = self.object_new(owner, std::iter::empty())?;
            self.groups.insert(value.bits, Group::default());
            return Ok(value);
        }
        let receiver = args[0].ok_or_else(|| Error("missing future group receiver".into()))?;
        self.object_descriptor(receiver)?;
        let group = self
            .groups
            .get(&receiver.bits)
            .ok_or_else(|| Error("Invalid future group".into()))?;
        match op {
            Operation::Allowed => {
                let origin = args[1].ok_or_else(|| Error("missing group future".into()))?;
                self.future(origin)?;
                Ok(Slot::boolean(
                    !group.sealed || group.entries.iter().any(|v| v.bits == origin.bits),
                ))
            }
            Operation::Register => {
                let origin = args[1].ok_or_else(|| Error("missing group future".into()))?;
                self.future(origin)?;
                let group = self.groups.get_mut(&receiver.bits).expect("group");
                if !group.entries.iter().any(|v| v.bits == origin.bits) {
                    if group.sealed {
                        return Err(Error("sealed group registration".into()));
                    }
                    group.entries.push(origin);
                    group.cancellation_owned.push(false);
                }
                Ok(origin)
            }
            Operation::Active => {
                let mut active = 0;
                for &entry in &group.entries {
                    if !matches!(self.future(entry)?.state, State::Ready(_)) {
                        active += 1;
                    }
                }
                Ok(Slot::small(active))
            }
            Operation::Cancel => {
                self.group_cancel(receiver)?;
                Ok(Slot::default())
            }
            Operation::Entries => {
                let group = self.groups.get_mut(&receiver.bits).expect("group");
                group.sealed = true;
                let entries = group.entries.clone();
                for &entry in &entries {
                    self.futures.get_mut(entry.bits)?.observed = true;
                }
                self.allocate_array(entries)
            }
            Operation::WaitFailed => {
                let failed = args[1].ok_or_else(|| Error("missing group failure".into()))?;
                let current = args[2].ok_or_else(|| Error("missing group child".into()))?;
                let external = !matches!(self.future(current)?.state,State::Ready(Err(f)) if f.bits==failed.bits);
                if external {
                    self.group_cancel(receiver)?;
                }
                Ok(Slot::boolean(external))
            }
            Operation::PreserveFailure => {
                let old = args[1].ok_or_else(|| Error("missing prior failure".into()))?;
                let new = args[2].ok_or_else(|| Error("missing new failure".into()))?;
                self.failure(new)?;
                Ok(if old.tag == FAILURE { old } else { new })
            }
            Operation::Finish => {
                let external = args[1].filter(|v| v.tag == FAILURE);
                let result = if let Some(cached) = group.result {
                    cached
                } else {
                    let entries = group.entries.clone();
                    let cancelled = group.cancellation_owned.clone();
                    let mut primary: Option<Slot> = None;
                    for (entry, owned) in entries.into_iter().zip(cancelled) {
                        let State::Ready(result) = self.future(entry)?.state else {
                            return Err(Error("group finished with running child".into()));
                        };
                        if let Err(failure) = result {
                            let record = self.failure(failure)?;
                            if owned
                                && matches!(record.kind, FailureKind::Cancelled)
                                && record.secondary.is_empty()
                            {
                                continue;
                            }
                            if let Some(first) = primary {
                                if first.bits != failure.bits {
                                    self.failure_link(first, failure, true)?;
                                }
                            } else {
                                primary = Some(failure);
                            }
                        }
                    }
                    let result = primary.map_or(Ok(receiver), Err);
                    self.groups.get_mut(&receiver.bits).expect("group").result = Some(result);
                    result
                };
                let result = if let Some(external) = external {
                    if let Err(failed) = result {
                        if failed.bits != external.bits {
                            self.failure_link(external, failed, true)?;
                        }
                    }
                    Err(external)
                } else {
                    result
                };
                self.future_with_state(State::Ready(result))
            }
            Operation::New(_) => unreachable!(),
        }
    }
}

pub(crate) fn lower(program: &crate::Program, index: usize) -> Result<crate::FunctionPlan, Error> {
    use crate::{Op, Type, array::ArrayOp, value::Arithmetic};
    use okkama_sparrow::{Cmp, Handler, Register, Target};
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("group intrinsic");
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    let params: Vec<_> = f.params.iter().map(|r| r.0 as usize).collect();
    let types: Vec<_> = params.iter().map(|&r| program.ty(f.registers[r])).collect();
    let result = f.result.and_then(|t| program.ty(t));
    let new = intrinsic.name == "okkama.scope.FutureGroup.new";
    let receiver = if new {
        result
    } else {
        types.first().copied().flatten()
    };
    let Some(Type::Object(owner)) = receiver else {
        return Err(invalid());
    };
    let object = &program.objects[owner.0 as usize];
    if !object.name.starts_with("async.scope::FutureGroup<")
        || !object.fields.is_empty()
        || object.destroy.is_some()
        || !intrinsic.helpers.is_empty()
    {
        return Err(invalid());
    }
    let output = f.registers.len();
    let mut handlers = vec![];
    let target = |pc| Target { pc, moves: vec![] };
    let op = |operation, dst, args| {
        Op::Intrinsic(crate::intrinsic::Operation::Group(operation), dst, args)
    };
    let first = [params.first().copied(), None, None];
    let (plan, registers, is_async) = match intrinsic.name.as_str() {
        "okkama.scope.FutureGroup.new" => {
            if !params.is_empty() || intrinsic.objects.len() != 1 || intrinsic.objects[0] != *owner
            {
                return Err(invalid());
            }
            (
                vec![
                    op(Operation::New(owner.0), output, [None; 3]),
                    Op::Return(output),
                ],
                output + 1,
                false,
            )
        }
        "okkama.scope.FutureGroup.start" => {
            if params.len() != 2
                || !matches!(types[1], Some(Type::Future(_)))
                || result.is_some()
                || intrinsic.objects.len() != 1
            {
                return Err(invalid());
            }
            let closed = intrinsic.objects[0].0;
            let object = &program.objects[closed as usize];
            if object.name != "async.scope::FinishedFutureGroup"
                || !object.fields.is_empty()
                || object.destroy.is_some()
            {
                return Err(invalid());
            }
            let args = [Some(params[0]), Some(params[1]), None];
            (
                vec![
                    op(Operation::Allowed, output, args),
                    Op::Branch(output, target(2), target(5)),
                    op(Operation::Register, output, args),
                    Op::FutureSpawn(output, params[1]),
                    Op::ReturnVoid,
                    Op::ObjectNew(output, closed, vec![]),
                    Op::Throw(output),
                ],
                output + 1,
                false,
            )
        }
        "okkama.scope.FutureGroup.active" | "okkama.scope.FutureGroup.cancel" => {
            if params.len() != 1 || !intrinsic.objects.is_empty() {
                return Err(invalid());
            }
            let active = intrinsic.name.ends_with(".active");
            if if active {
                result != Some(&Type::Int)
            } else {
                result.is_some()
            } {
                return Err(invalid());
            }
            (
                vec![
                    op(
                        if active {
                            Operation::Active
                        } else {
                            Operation::Cancel
                        },
                        output,
                        first,
                    ),
                    if active {
                        Op::Return(output)
                    } else {
                        Op::ReturnVoid
                    },
                ],
                output + 1,
                false,
            )
        }
        "okkama.scope.FutureGroup.wait" => {
            if params.len() != 1 || result != receiver || !intrinsic.objects.is_empty() {
                return Err(invalid());
            }
            let (entries, i, len, cond, child, failure, external, one, finished) = (
                output,
                output + 1,
                output + 2,
                output + 3,
                output + 4,
                output + 5,
                output + 6,
                output + 7,
                output + 8,
            );
            handlers.push(Handler {
                start: 8,
                end: 9,
                target: target(11),
                failure: Register(failure as u32),
            });
            (
                vec![
                    op(Operation::Entries, entries, first),
                    Op::Const(i, Slot::small(0)),
                    Op::Const(external, Slot::boolean(false)),
                    Op::Const(one, Slot::small(1)),
                    Op::Array(ArrayOp::Length, len, [Some(entries), None, None]),
                    Op::Compare(Cmp::Lt, cond, i, len),
                    Op::Branch(cond, target(7), target(15)),
                    Op::Array(ArrayOp::Get, child, [Some(entries), Some(i), None]),
                    Op::Await {
                        dst: None,
                        future: child,
                    },
                    Op::Arithmetic(Arithmetic::Add, i, i, one),
                    Op::Jump(target(5)),
                    op(
                        Operation::WaitFailed,
                        cond,
                        [Some(params[0]), Some(failure), Some(child)],
                    ),
                    Op::Branch(cond, target(13), target(9)),
                    Op::Copy(external, failure),
                    Op::Jump(target(5)),
                    op(
                        Operation::Finish,
                        finished,
                        [Some(params[0]), Some(external), None],
                    ),
                    Op::Await {
                        dst: None,
                        future: finished,
                    },
                    Op::Return(params[0]),
                ],
                output + 9,
                true,
            )
        }
        _ => return Err(invalid()),
    };
    if f.is_async != is_async {
        return Err(invalid());
    }
    let live = crate::liveness::analyze(&plan, &handlers);
    let call_saved = crate::liveness::call_saved(&plan, &live, &handlers);
    Ok(crate::FunctionPlan {
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
        handlers,
    })
}

pub(crate) fn lower_scope(
    program: &crate::Program,
    index: usize,
) -> Result<crate::FunctionPlan, Error> {
    use crate::{Op, Type, array::ArrayOp, plan::Plan, value::Arithmetic};
    use okkama_sparrow::{Cmp, Handler, Register, Target};
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("scope intrinsic");
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    if !f.is_async
        || f.params.len() != 1
        || intrinsic.objects.len() != 1
        || !intrinsic.helpers.is_empty()
    {
        return Err(invalid());
    }
    let receiver = f.params[0].0 as usize;
    let Some(Type::Object(owner)) = program.ty(f.registers[receiver]) else {
        return Err(invalid());
    };
    let object = &program.objects[owner.0 as usize];
    if !object.name.starts_with("async.scope::FutureScope<")
        || object.fields.len() != 1
        || object.fields[0].0 != "action"
        || object.destroy.is_some()
    {
        return Err(invalid());
    }
    let Some(Type::Fn {
        params,
        result: Some(action_result),
    }) = program.ty(object.fields[0].1)
    else {
        return Err(invalid());
    };
    if params.len() != 1 || program.ty(*action_result) != Some(&Type::Future(f.result)) {
        return Err(invalid());
    }
    let Some(Type::Role(supervision)) = program.ty(params[0]) else {
        return Err(invalid());
    };
    let group_owner = intrinsic.objects[0].0;
    let group_type = &program.objects[group_owner as usize];
    if !group_type.name.starts_with("async.scope::FutureGroup<")
        || !group_type.fields.is_empty()
        || group_type.destroy.is_some()
        || !group_type.does.iter().any(|w| w.role == *supervision)
        || !program.roles[supervision.0 as usize]
            .name
            .starts_with("async.scope::Supervising<")
    {
        return Err(invalid());
    }
    for slot in 0..program.roles[supervision.0 as usize].methods.len() {
        if !crate::validate::role_witnesses(program, supervision.0, slot) {
            return Err(invalid());
        }
    }
    let n = f.registers.len();
    let (
        group,
        action,
        body_future,
        body_value,
        external,
        entries,
        i,
        len,
        one,
        cond,
        child,
        failure,
        finished,
    ) = (
        n,
        n + 1,
        n + 2,
        n + 3,
        n + 4,
        n + 5,
        n + 6,
        n + 7,
        n + 8,
        n + 9,
        n + 10,
        n + 11,
        n + 12,
    );
    let mut p = Plan::default();
    let op = |operation, dst, args| {
        Op::Intrinsic(crate::intrinsic::Operation::Group(operation), dst, args)
    };
    let first = [Some(group), None, None];
    p.push(op(Operation::New(group_owner), group, [None; 3]));
    p.push(Op::Const(external, Slot::boolean(false)));
    p.push(Op::ObjectGet(action, receiver, 0));
    let body_start = p.ops.len() as u32;
    p.push(Op::CallFn {
        dst: Some(body_future),
        callee: action,
        args: vec![group],
        tail: false,
    });
    p.push(Op::Await {
        dst: f.result.map(|_| body_value),
        future: body_future,
    });
    let body_end = p.ops.len() as u32;
    p.jump("wait_init");
    let body_failed = p.ops.len() as u32;
    p.push(Op::Copy(external, failure));
    p.push(op(Operation::Cancel, finished, first));
    p.jump("wait_init");
    p.label("wait_init");
    p.push(op(Operation::Entries, entries, first));
    p.push(Op::Const(i, Slot::small(0)));
    p.push(Op::Const(one, Slot::small(1)));
    p.push(Op::Array(ArrayOp::Length, len, [Some(entries), None, None]));
    p.label("wait");
    p.push(Op::Compare(Cmp::Lt, cond, i, len));
    p.branch(cond, "child", "finish");
    p.label("child");
    p.push(Op::Array(
        ArrayOp::Get,
        child,
        [Some(entries), Some(i), None],
    ));
    let wait_pc = p.ops.len() as u32;
    p.push(Op::Await {
        dst: None,
        future: child,
    });
    p.label("advance");
    p.push(Op::Arithmetic(Arithmetic::Add, i, i, one));
    p.jump("wait");
    let wait_failed = p.ops.len() as u32;
    p.push(op(
        Operation::WaitFailed,
        cond,
        [Some(group), Some(failure), Some(child)],
    ));
    p.branch(cond, "external", "advance");
    p.label("external");
    p.push(op(
        Operation::PreserveFailure,
        external,
        [Some(group), Some(external), Some(failure)],
    ));
    p.jump("wait");
    p.label("finish");
    p.push(op(
        Operation::Finish,
        finished,
        [Some(group), Some(external), None],
    ));
    p.push(Op::Await {
        dst: None,
        future: finished,
    });
    p.push(if f.result.is_some() {
        Op::Return(body_value)
    } else {
        Op::ReturnVoid
    });
    let plan = p.finish();
    let handlers = vec![
        Handler {
            start: body_start,
            end: body_end,
            target: Target {
                pc: body_failed,
                moves: vec![],
            },
            failure: Register(failure as u32),
        },
        Handler {
            start: wait_pc,
            end: wait_pc + 1,
            target: Target {
                pc: wait_failed,
                moves: vec![],
            },
            failure: Register(failure as u32),
        },
    ];
    let live = crate::liveness::analyze(&plan, &handlers);
    let call_saved = crate::liveness::call_saved(&plan, &live, &handlers);
    Ok(crate::FunctionPlan {
        native: None,
        is_async: true,
        cold_entry: 0,
        cached: true,
        symbol: f.symbol.clone(),
        recurrence_loop: false,
        elided_objects: 0,
        params: vec![receiver],
        plan,
        registers: n + 13,
        compiled: Default::default(),
        jit_kinds: None,
        live,
        call_saved,
        handlers,
    })
}
