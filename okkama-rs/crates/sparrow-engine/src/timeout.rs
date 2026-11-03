//! Timeout policy uses the same Tokio-driven completion notifications as races.
//! Clock methods remain ordinary language calls, including injected test clocks.
use crate::{
    Error, FunctionPlan, Op, Program, Type,
    collect::{Operation, Strategy},
    plan::Plan,
    value::Slot,
};
use okkama_sparrow::{Cmp, Handler, Register, Target};

pub(crate) fn lower(program: &Program, index: usize) -> Result<FunctionPlan, Error> {
    let f = &program.functions[index];
    let intrinsic = f.intrinsic.as_ref().expect("timeout intrinsic");
    let invalid = || {
        Error(format!(
            "invalid intrinsic signature/layout: {}",
            intrinsic.name
        ))
    };
    let before = match intrinsic.name.as_str() {
        "okkama.timeout.Within.result" => false,
        "okkama.timeout.Before.result" => true,
        _ => return Err(invalid()),
    };
    if !f.is_async
        || f.params.len() != 1
        || intrinsic.objects.len() != 1
        || intrinsic.helpers.len() != if before { 4 } else { 2 }
    {
        return Err(invalid());
    }
    let receiver = f.params[0].0 as usize;
    let Some(Type::Object(owner)) = program.ty(f.registers[receiver]) else {
        return Err(invalid());
    };
    let object = &program.objects[owner.0 as usize];
    let prefix = if before {
        "async.timeout::Before<"
    } else {
        "async.timeout::Within<"
    };
    if !object.name.starts_with(prefix)
        || object.destroy.is_some()
        || object.fields.len() != 3
        || object.fields[0].0 != "origin"
        || object.fields[1].0 != if before { "deadline" } else { "limit" }
        || object.fields[2].0 != "clock"
        || program.ty(object.fields[0].1) != Some(&Type::Future(f.result))
    {
        return Err(invalid());
    }
    let timed = intrinsic.objects[0].0;
    let error = &program.objects[timed as usize];
    if error.name != "async.timeout::TimedOut"
        || !error.fields.is_empty()
        || error.destroy.is_some()
    {
        return Err(invalid());
    }
    let helpers: Vec<_> = intrinsic.helpers.iter().map(|h| h.0 as usize).collect();
    let offset = if before { 2 } else { 0 };
    let seconds = &program.functions[helpers[offset]];
    let wait = &program.functions[helpers[offset + 1]];
    let signature = |function: &okkama_sparrow::Function,
                     params: &[okkama_sparrow::TypeRef],
                     result: Option<okkama_sparrow::TypeRef>| {
        !function.is_async
            && function.params.len() == params.len()
            && function
                .params
                .iter()
                .zip(params)
                .all(|(r, t)| function.registers[r.0 as usize] == *t)
            && function.result == result
    };
    if seconds.params.len() != 1
        || seconds.is_async
        || seconds.result.and_then(|t| program.ty(t)) != Some(&Type::Rat)
    {
        return Err(invalid());
    }
    let duration = seconds.registers[seconds.params[0].0 as usize];
    if !matches!(program.ty(duration), Some(Type::Object(_)))
        || wait.result.and_then(|t| program.ty(t)) != Some(&Type::Future(None))
        || !signature(wait, &[object.fields[2].1, duration], wait.result)
    {
        return Err(invalid());
    }
    if before {
        let now = &program.functions[helpers[0]];
        let minus = &program.functions[helpers[1]];
        if !signature(now, &[object.fields[2].1], Some(object.fields[1].1))
            || !signature(
                minus,
                &[object.fields[1].1, object.fields[1].1],
                Some(duration),
            )
        {
            return Err(invalid());
        }
    } else if object.fields[1].1 != duration {
        return Err(invalid());
    }
    let n = f.registers.len();
    let (
        origin,
        clock,
        limit,
        amount,
        zero,
        timer,
        cond,
        entries,
        pending,
        position,
        result,
        failure,
        scratch,
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
    let operation = |op, dst, arg| {
        Op::Intrinsic(
            crate::intrinsic::Operation::Collect(op),
            dst,
            [Some(receiver), arg, None],
        )
    };
    let call = |function, dst, args| Op::Call {
        function,
        dst: Some(dst),
        args,
        tail: false,
    };
    p.push(Op::ObjectGet(origin, receiver, 0));
    p.push(Op::FutureSpawn(origin, origin));
    p.push(Op::FutureControl(
        okkama_sparrow::FutOp::Done,
        Some(cond),
        origin,
    ));
    p.branch(cond, "ready", "timer");
    p.label("ready");
    p.push(Op::Await {
        dst: Some(result),
        future: origin,
    });
    p.push(Op::Return(result));
    p.label("timer");
    let setup_start = p.ops.len() as u32;
    p.push(Op::ObjectGet(clock, receiver, 2));
    p.push(Op::ObjectGet(limit, receiver, 1));
    if before {
        p.push(call(helpers[0], scratch, vec![clock]));
        p.push(call(helpers[1], limit, vec![limit, scratch]));
    }
    p.push(call(helpers[offset], amount, vec![limit]));
    p.push(Op::Const(zero, Slot::small(0)));
    p.push(Op::Convert(crate::scalar::Kind::Rat, zero, zero));
    p.push(Op::Compare(Cmp::Gt, cond, amount, zero));
    p.branch(cond, "wait_clock", "expired");
    p.label("wait_clock");
    p.push(call(helpers[offset + 1], timer, vec![clock, limit]));
    p.push(Op::FutureSpawn(timer, timer));
    let setup_end = p.ops.len() as u32;
    p.jump("race");
    p.label("expired");
    p.push(Op::Const(scratch, Slot::boolean(true)));
    p.push(operation(Operation::Settled(true), timer, Some(scratch)));
    p.jump("race");
    let setup_failed = p.ops.len() as u32;
    p.push(operation(Operation::Settled(false), timer, Some(failure)));
    p.label("race");
    p.push(Op::ArrayNew(entries, vec![origin, timer]));
    p.push(operation(
        Operation::Init(Strategy::Timeout(timed)),
        scratch,
        Some(entries),
    ));
    p.push(operation(Operation::Begin, scratch, None));
    p.label("next");
    p.push(operation(Operation::Next, pending, None));
    let await_pc = p.ops.len() as u32;
    p.push(Op::Await {
        dst: Some(position),
        future: pending,
    });
    p.push(Op::Const(zero, Slot::small(0)));
    p.push(Op::Compare(Cmp::Ge, cond, position, zero));
    p.branch(cond, "consume", "finish");
    p.label("consume");
    p.push(operation(Operation::Consume, scratch, Some(position)));
    p.jump("next");
    p.label("finish");
    p.push(operation(Operation::Finish, pending, None));
    p.push(Op::Await {
        dst: Some(result),
        future: pending,
    });
    p.push(Op::Return(result));
    let cancel_pc = p.ops.len() as u32;
    p.push(operation(Operation::Abort, scratch, Some(failure)));
    p.jump("next");
    let handlers = vec![
        Handler {
            start: setup_start,
            end: setup_end,
            target: Target {
                pc: setup_failed,
                moves: vec![],
            },
            failure: Register(failure as u32),
        },
        Handler {
            start: await_pc,
            end: await_pc + 1,
            target: Target {
                pc: cancel_pc,
                moves: vec![],
            },
            failure: Register(failure as u32),
        },
    ];
    let plan = p.finish();
    let live = crate::liveness::analyze(&plan, &handlers);
    let call_saved = crate::liveness::call_saved(&plan, &live, &handlers);
    Ok(FunctionPlan {
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
