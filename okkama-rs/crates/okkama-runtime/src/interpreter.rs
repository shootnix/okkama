//! Обход проверенного IR: кадры, чтение операндов, вызовы и переходы.
use crate::{
    RuntimeError, invalid_state, operations,
    state::State,
    unsupported,
    value::{RuntimeValue, Value},
};
use okkama_compiler::{
    ir::{self, InstKind, IntValue, TermKind},
    source::Span,
};
use okkama_runtime_core::failure::{Failure, Kind as FailureKind};
use okkama_runtime_core::sched;
use std::rc::Rc;

/// Проверяет IR и исполняет его точку входа. Не печатает и не завершает процесс.
/// Каждый вызов создаёт собственные слоты, не сохраняя глобального состояния.
pub fn execute(module: &ir::Module) -> Result<IntValue, RuntimeError> {
    execute_with_arguments(module, Vec::new())
}

/// То же с аргументами командной строки программы.
pub fn execute_with_arguments(
    module: &ir::Module,
    arguments: Vec<String>,
) -> Result<IntValue, RuntimeError> {
    ir::verify(module).map_err(RuntimeError::InvalidIr)?;
    if !module.imports.is_empty() || module.tests.is_some() {
        return Err(unsupported(
            module.span,
            "unresolved imports and test mode are not supported yet",
        ));
    }
    let entry = module.entry.ok_or(RuntimeError::MissingEntry)?;
    let engine = new_engine(module);
    if crate::worker::active() {
        crate::worker::serve(&engine);
    }
    engine.state.set_arguments(arguments);
    sched::reset();
    let result = sched::on_large_stack(|| run_entry(&engine, entry));
    // Незавершённые задачи освобождаются до возврата: их стеки держат значения.
    sched::reset();
    take_fatal(&engine)?;
    result
}

pub(crate) fn new_engine(module: &ir::Module) -> Rc<Engine> {
    let engine = Rc::new(Engine {
        module: module.clone(),
        state: State::default(),
        fatal: std::cell::RefCell::new(None),
    });
    engine.state.set_enums(module);
    engine
}

/// Ошибка реализации, случившаяся внутри задачи.
pub(crate) fn take_fatal(engine: &Engine) -> Result<(), RuntimeError> {
    match engine.fatal.borrow_mut().take() {
        Some(fatal) => Err(fatal),
        None => Ok(()),
    }
}

/// Ошибка исполнения как отказ программы; ошибки реализации остаются собой.
pub(crate) fn into_failure(error: RuntimeError) -> Result<Rc<Failure>, RuntimeError> {
    from_error(error)
}

/// Исполнитель одного запуска: модуль и состояние, общие для задач.
#[derive(Debug)]
pub(crate) struct Engine {
    pub(crate) module: ir::Module,
    pub(crate) state: State,
    /// Ошибка реализации внутри задачи: завершает запуск после планировщика.
    fatal: std::cell::RefCell<Option<RuntimeError>>,
}

fn run_entry(engine: &Rc<Engine>, entry: ir::FuncId) -> Result<IntValue, RuntimeError> {
    let module = &engine.module;
    let outcome = if module.functions[entry.0].is_async() {
        // `async main`: корневой Future и цикл планировщика до его исхода.
        let root = cold_call(engine, entry, Vec::new());
        sched::run_main(&root)
            .map(Some)
            .map_err(|failure| failure_error(failure, module.span))
    } else {
        execute_function(engine, entry, Vec::new())
    };
    match outcome {
        Ok(value) => value
            .ok_or_else(|| invalid_state(module.span, "entry returned no value"))?
            .int(module.span)
            .cloned(),
        Err(RuntimeError::Unwind { span, failure }) => Err(RuntimeError::Unwind {
            span,
            failure: describe_failure(engine, &failure, span)?,
        }),
        Err(error) => Err(error),
    }
}

/// Вызов async-функции: холодный Future над её телом и аргументами.
pub(crate) fn cold_call(engine: &Rc<Engine>, func: ir::FuncId, args: Vec<Value>) -> sched::Future {
    let body_engine = engine.clone();
    let retained = args.clone();
    sched::cold(
        Box::new(move || run_body(&body_engine, func, args)),
        retained,
    )
}

/// Тело задачи: исход вызова; ошибка реализации запоминается для запуска.
pub(crate) fn run_body(engine: &Rc<Engine>, func: ir::FuncId, args: Vec<Value>) -> sched::Outcome {
    match execute_function(engine, func, args) {
        Ok(value) => Ok(value.unwrap_or(Value::Undef)),
        Err(error) => match from_error(error) {
            Ok(failure) => Err(failure),
            Err(fatal) => {
                let message = fatal.to_string();
                engine.fatal.borrow_mut().get_or_insert(fatal);
                Err(Failure::new(FailureKind::Program(message)))
            }
        },
    }
}

pub(crate) fn execute_function(
    engine: &Rc<Engine>,
    entry: ir::FuncId,
    args: Vec<Value>,
) -> Result<Option<Value>, RuntimeError> {
    let module = &engine.module;
    let state = &engine.state;
    let mut frames = vec![Frame::new(module, entry, args, None, module.span)?];
    loop {
        let frame = frames
            .last_mut()
            .expect("entry returns before the stack is empty");
        let function = &module.functions[frame.function.0];
        let block = &function.blocks[frame.block.0];
        let slots = &mut frame.slots;
        if let Some(inst) = block.insts.get(frame.next_inst) {
            // Продолжение вызова — следующая инструкция, а не начало блока.
            frame.next_inst += 1;
            // Вызов слота роли идёт общим путём вызова: witness может быть
            // async-, native- или intrinsic-функцией.
            let mut role_call = None;
            if let InstKind::CallRole {
                receiver,
                slot,
                args,
            } = &inst.kind
            {
                let ir::Type::Role(role) = function.values[receiver.0] else {
                    return Err(invalid_state(inst.span, "role receiver type is missing"));
                };
                let receiver = read(slots, *receiver, inst.span)?;
                if let Value::NativeFailure(failure) = receiver {
                    let declaration = &module.roles[role.0];
                    if !declaration.is_error()
                        || declaration
                            .methods
                            .get(*slot)
                            .is_none_or(|m| m.name != "text")
                        || !args.is_empty()
                    {
                        return Err(invalid_state(inst.span, "invalid native Error role call"));
                    }
                    let result = inst
                        .result
                        .ok_or_else(|| invalid_state(inst.span, "Error.text requires result"))?;
                    slots[result.0] = Some(Value::Str(Rc::from(failure.message.as_str())));
                    continue;
                }
                let witness = state.role_witness(module, receiver, role, inst.span)?;
                let Some(witness) = witness else {
                    // `Cancelled` рантайма в программе без его объявления: только Error.text.
                    if is_runtime_cancelled(receiver) && module.roles[role.0].is_error() {
                        if let Some(result) = inst.result {
                            slots[result.0] = Some(Value::Str(Rc::from("Operation was cancelled")));
                        }
                        continue;
                    }
                    return Err(invalid_state(inst.span, "role witness is missing"));
                };
                let callee = *witness
                    .slots
                    .get(*slot)
                    .ok_or_else(|| invalid_state(inst.span, "role slot is missing"))?;
                let arguments = std::iter::once(Ok(receiver.clone()))
                    .chain(args.iter().map(|id| read(slots, *id, inst.span).cloned()))
                    .collect::<Result<Vec<_>, _>>()?;
                role_call = Some((callee, arguments));
            }
            if let InstKind::RegionLeave { region, .. } | InstKind::RegionUnwind { region, .. } =
                &inst.kind
            {
                let Value::Region(region) = read(slots, *region, inst.span)? else {
                    return Err(invalid_state(inst.span, "expected region value"));
                };
                let region = region.clone();
                if frame.closing.is_none() {
                    let keep = match &inst.kind {
                        InstKind::RegionLeave { keep, .. } => keep
                            .map(|id| read(slots, id, inst.span).cloned())
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                        // Error-объекты отказа освобождаются (`Region.unwind`).
                        InstKind::RegionUnwind { failure: id, .. } => {
                            thrown_errors(failure(read(slots, *id, inst.span)?, inst.span)?)
                        }
                        _ => unreachable!("matched above"),
                    };
                    frame.closing = Some(PendingClose {
                        closing: region.begin_close(&keep),
                        failures: Vec::new(),
                    });
                }
                let pending = frame.closing.as_mut().expect("closing started above");
                let mut destroy = None;
                while let Some(object) = region.next_destroyed(&mut pending.closing) {
                    if let Some(body) = module.objects.get(object.declaration).and_then(|o| o.destroy) {
                        destroy = Some((body, object));
                        break;
                    }
                }
                if let Some((body, object)) = destroy {
                    // Инструкция исполнится заново, когда `destroy` завершится.
                    frame.next_inst -= 1;
                    let mut callee =
                        Frame::new(module, body, vec![Value::Object(object)], None, inst.span)?;
                    callee.boundary = true;
                    frames.push(callee);
                    continue;
                }
                let failures = frame.closing.take().expect("closing started above").failures;
                match &inst.kind {
                    InstKind::RegionUnwind { failure: id, .. } => {
                        // Отказы destroy на пути отказа — вторичные к нему.
                        let mut active = failure(read(slots, *id, inst.span)?, inst.span)?.clone();
                        for cleanup in failures {
                            active = active.with_secondary(cleanup);
                        }
                        let result = inst
                            .result
                            .ok_or_else(|| invalid_state(inst.span, "region.unwind has no result"))?;
                        slots[result.0] = Some(Value::Failure(active));
                    }
                    _ => {
                        // Первый отказ destroy — основной, остальные — вторичные.
                        let mut failures = failures.into_iter();
                        if let Some(mut primary) = failures.next() {
                            for cleanup in failures {
                                primary = primary.with_secondary(cleanup);
                            }
                            propagate(module, &mut frames, primary, inst.span)?;
                        }
                    }
                }
                continue;
            }
            let call = match &inst.kind {
                _ if role_call.is_some() => role_call,
                InstKind::Call { func, args } => Some((
                    *func,
                    args.iter()
                        .map(|id| read(slots, *id, inst.span).cloned())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                // Замыкание: функция над захватами, затем аргументами.
                InstKind::CallFn { callee, args } => {
                    let Value::Fn(closure) = read(slots, *callee, inst.span)? else {
                        return Err(invalid_state(inst.span, "call.fn of a non-function value"));
                    };
                    let mut arguments = closure.captures.clone();
                    for id in args {
                        arguments.push(read(slots, *id, inst.span)?.clone());
                    }
                    Some((ir::FuncId(closure.function), arguments))
                }
                // Поле cap вычисляется заново над владельцем (§8.5).
                InstKind::CallCap { cap, field } => {
                    let ir::Type::Cap(id) = function.values[cap.0] else {
                        return Err(invalid_state(inst.span, "call.cap of a non-cap value"));
                    };
                    let func = module
                        .cap(id)
                        .and_then(|cap| cap.fields.get(*field))
                        .ok_or_else(|| invalid_state(inst.span, "cap field is missing"))?
                        .1;
                    Some((func, vec![read(slots, *cap, inst.span)?.clone()]))
                }
                _ => None,
            };
            if let Some((func, arguments)) = call {
                let func = &func;
                let function = &module.functions[func.0];
                if let Some((name, objects)) = function.intrinsic() {
                    let host: okkama_runtime_core::intrinsics::HostRef =
                        Rc::new(crate::host::IrHost(engine.clone()));
                    let objects: Vec<usize> = objects.iter().map(|object| object.0).collect();
                    let helpers: Vec<usize> =
                        function.intrinsic_helpers().iter().map(|f| f.0).collect();
                    match okkama_runtime_core::intrinsics::call(
                        &host, name, &objects, &helpers, arguments,
                    ) {
                        Ok(value) => {
                            if let Some(result) = inst.result {
                                slots[result.0] = Some(value);
                            }
                        }
                        Err(raised) => propagate(module, &mut frames, raised, inst.span)?,
                    }
                    continue;
                }
                if function.is_async() && function.extern_symbol().is_none() {
                    let future = cold_call(engine, *func, arguments);
                    if let Some(result) = inst.result {
                        slots[result.0] = Some(Value::Future(future));
                    }
                    continue;
                }
                {
                    if let Some(symbol) = function.extern_symbol() {
                        validate_native_signature(function, inst.span)?;
                        let value = match state.native(symbol, &arguments, inst.span) {
                            Ok(value) => value,
                            Err(error) => {
                                let raised = from_error(error)?;
                                propagate(module, &mut frames, raised, inst.span)?;
                                continue;
                            }
                        };
                        if let Some(result) = inst.result {
                            slots[result.0] = Some(value);
                        }
                        continue;
                    }
                }
                let callee = Frame::new(module, *func, arguments, inst.result, inst.span)?;
                frames.push(callee);
                continue;
            }
            let evaluated = (|| -> Result<Value, RuntimeError> {
                Ok(match &inst.kind {
                    InstKind::FailSecondary { primary, cleanup } => Value::Failure(
                        failure(read(slots, *primary, inst.span)?, inst.span)?.with_secondary(
                            failure(read(slots, *cleanup, inst.span)?, inst.span)?.clone(),
                        ),
                    ),
                    InstKind::FailCaused { new, handled } => Value::Failure(Failure::caused(
                        failure(read(slots, *new, inst.span)?, inst.span)?,
                        failure(read(slots, *handled, inst.span)?, inst.span)?,
                    )),
                    InstKind::RoleWrap(source) | InstKind::RoleNarrow { value: source, .. } => {
                        let ir::Type::Role(role) =
                            function.values[inst.result.expect("verified role result").0]
                        else {
                            return Err(invalid_state(inst.span, "role result type is missing"));
                        };
                        let value = read(slots, *source, inst.span)?;
                        if matches!(value, Value::NativeFailure(_)) {
                            if module.roles[role.0].is_error() {
                                return Ok(value.clone());
                            }
                            return Err(match &inst.kind {
                                InstKind::RoleNarrow { message, .. } => RuntimeError::Trap {
                                    span: inst.span,
                                    message: message.clone(),
                                },
                                _ => {
                                    invalid_state(inst.span, "NativeFailure only implements Error")
                                }
                            });
                        }
                        if state
                            .role_witness(module, value, role, inst.span)?
                            .is_none()
                        {
                            return Err(match &inst.kind {
                                InstKind::RoleNarrow { message, .. } => RuntimeError::Trap {
                                    span: inst.span,
                                    message: message.clone(),
                                },
                                _ => invalid_state(inst.span, "role witness is missing"),
                            });
                        }
                        value.clone()
                    }
                    InstKind::RangeNew {
                        start,
                        end,
                        inclusive,
                    } => Value::Range(Rc::new(
                        okkama_runtime_core::range::Range::new(
                            read(slots, *start, inst.span)?.int(inst.span)?.clone(),
                            read(slots, *end, inst.span)?.int(inst.span)?.clone(),
                            *inclusive,
                        )
                        .map_err(|message| RuntimeError::Trap {
                            span: inst.span,
                            message: message.into(),
                        })?,
                    )),
                    InstKind::RangePart { range, end } => {
                        let range = read(slots, *range, inst.span)?
                            .as_range()
                            .map_err(|e| invalid_state(inst.span, e.message))?;
                        Value::Int(if *end {
                            range.end_exclusive()
                        } else {
                            range.start()
                        })
                    }
                    InstKind::ValCompare { op, left, right } => {
                        let equal = state.equal(
                            read(slots, *left, inst.span)?,
                            read(slots, *right, inst.span)?,
                            inst.span,
                        )?;
                        Value::Bool(if *op == ir::CompareOp::Eq {
                            equal
                        } else {
                            !equal
                        })
                    }
                    InstKind::EnumCase { ty, case } => Value::Enum {
                        declaration: ty.0,
                        case: *case,
                    },
                    InstKind::EnumCompare { op, left, right } => {
                        let equal = read(slots, *left, inst.span)?
                            .enum_eq(read(slots, *right, inst.span)?)
                            .map_err(|e| invalid_state(inst.span, e.message))?;
                        Value::Bool(if *op == ir::CompareOp::Eq {
                            equal
                        } else {
                            !equal
                        })
                    }
                    InstKind::ByteBinary { op, left, right } => operations::byte_binary(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::ByteNot(value) => Value::Byte(
                        !read(slots, *value, inst.span)?
                            .as_byte()
                            .map_err(|e| invalid_state(inst.span, e.message))?,
                    ),
                    InstKind::BuilderNew => state.builder_new(),
                    InstKind::BuilderAppendRange {
                        builder,
                        start,
                        end,
                        inclusive,
                    } => state.builder_range(
                        read(slots, *builder, inst.span)?,
                        read(slots, *start, inst.span)?.int(inst.span)?,
                        read(slots, *end, inst.span)?.int(inst.span)?,
                        *inclusive,
                        inst.span,
                    )?,
                    InstKind::BuilderPush { builder, value } => state.builder_push(
                        read(slots, *builder, inst.span)?,
                        read(slots, *value, inst.span)?.clone(),
                        inst.span,
                    )?,
                    InstKind::BuilderAppend { builder, array } => state.builder_append(
                        read(slots, *builder, inst.span)?,
                        read(slots, *array, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::BuilderFinish(builder) => {
                        state.builder_finish(read(slots, *builder, inst.span)?, inst.span)?
                    }
                    InstKind::Cvt(value) => operations::convert(
                        read(slots, *value, inst.span)?,
                        &function.values[inst.result.expect("verified conversion result").0],
                        inst.span,
                    )?,
                    InstKind::ByteFrom { value, context } => operations::byte_from(
                        read(slots, *value, inst.span)?,
                        context.as_deref(),
                        inst.span,
                    )?,
                    InstKind::ByteCompare { op, left, right }
                    | InstKind::CharCompare { op, left, right } => operations::scalar_compare(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::Str {
                        op: ir::StrOp::Utf8,
                        args,
                    } => state.str_utf8(read(slots, args[0], inst.span)?.string(inst.span)?),
                    InstKind::ArraySlice { array, start, end } => state.array_slice(
                        read(slots, *array, inst.span)?,
                        read(slots, *start, inst.span)?.int(inst.span)?,
                        read(slots, *end, inst.span)?.int(inst.span)?,
                        inst.span,
                    )?,
                    InstKind::ArrayNew(elements) => state.array_new(
                        elements
                            .iter()
                            .map(|id| read(slots, *id, inst.span).cloned())
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    InstKind::ArrayLen(array) => {
                        state.array_len(read(slots, *array, inst.span)?, inst.span)?
                    }
                    InstKind::ArrayGet { array, index } => state.array_get(
                        read(slots, *array, inst.span)?,
                        read(slots, *index, inst.span)?.int(inst.span)?,
                        inst.span,
                    )?,
                    InstKind::Retag(value) | InstKind::SuccTake(value) => {
                        read(slots, *value, inst.span)?.clone()
                    }
                    InstKind::ObjNew { object, fields } => state.object_new(
                        *object,
                        &module.objects[object.0],
                        fields
                            .iter()
                            .map(|id| read(slots, *id, inst.span).cloned())
                            .collect::<Result<Vec<_>, _>>()?,
                        inst.span,
                    )?,
                    InstKind::ObjGet { object, field } => {
                        state.object_get(read(slots, *object, inst.span)?, *field, inst.span)?
                    }
                    InstKind::ObjNext { object, updates } => {
                        let updates = updates
                            .iter()
                            .map(|(field, id)| Ok((*field, read(slots, *id, inst.span)?.clone())))
                            .collect::<Result<Vec<_>, RuntimeError>>()?;
                        state.object_next(read(slots, *object, inst.span)?, updates, inst.span)?
                    }
                    InstKind::Undef => Value::Undef,
                    InstKind::Const(value) => operations::constant(value, inst.span)?,
                    InstKind::Str {
                        op: ir::StrOp::Split,
                        args,
                    } => state.str_split(
                        read(slots, args[0], inst.span)?.string(inst.span)?,
                        read(slots, args[1], inst.span)?.string(inst.span)?,
                    ),
                    InstKind::ArrayJoin { array, separator } => state.array_join(
                        read(slots, *array, inst.span)?,
                        read(slots, *separator, inst.span)?.string(inst.span)?,
                        inst.span,
                    )?,
                    InstKind::Str { op, args } => {
                        let args = args
                            .iter()
                            .map(|id| read(slots, *id, inst.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        operations::string(*op, &args, inst.span)?
                    }
                    InstKind::StrConcat { left, right } => operations::concat(
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::StrCompare { op, left, right } => operations::string_compare(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::IntBinary { op, left, right } => operations::int_binary(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::IntUnary { op, operand } => {
                        operations::int_unary(*op, read(slots, *operand, inst.span)?, inst.span)?
                    }
                    InstKind::IntCompare { op, left, right } => operations::int_compare(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::Float { op, args } => {
                        let args = args
                            .iter()
                            .map(|id| read(slots, *id, inst.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        operations::float(*op, &args, inst.span)?
                    }
                    InstKind::FloatCompare { op, left, right } => operations::float_compare(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::Rat { op, args } => {
                        let args = args
                            .iter()
                            .map(|id| read(slots, *id, inst.span))
                            .collect::<Result<Vec<_>, _>>()?;
                        operations::rat(*op, &args, inst.span)?
                    }
                    InstKind::RatCompare { op, left, right } => operations::rat_compare(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::RatPart { value, denominator } => operations::rat_part(
                        read(slots, *value, inst.span)?,
                        *denominator,
                        inst.span,
                    )?,
                    InstKind::FloatSign(value)
                    | InstKind::RatSign(value)
                    | InstKind::IntSign(value) => {
                        operations::sign(read(slots, *value, inst.span)?, inst.span)?
                    }
                    InstKind::IntAbs(value) => {
                        operations::int_abs(read(slots, *value, inst.span)?, inst.span)?
                    }
                    InstKind::Bits {
                        op,
                        byte,
                        value,
                        count,
                    } => operations::bits(
                        *op,
                        *byte,
                        read(slots, *value, inst.span)?,
                        count.map(|count| read(slots, count, inst.span)).transpose()?,
                        inst.span,
                    )?,
                    InstKind::Await(future) => {
                        let Value::Future(future) = read(slots, *future, inst.span)? else {
                            return Err(invalid_state(inst.span, "await of a non-future value"));
                        };
                        let future = future.clone();
                        sched::await_future(&future).map_err(|failure| RuntimeError::Unwind {
                            span: inst.span,
                            failure,
                        })?
                    }
                    InstKind::FutSpawn(future) => {
                        let value = read(slots, *future, inst.span)?.clone();
                        let Value::Future(future) = &value else {
                            return Err(invalid_state(inst.span, "spawn of a non-future value"));
                        };
                        sched::spawn(future);
                        value
                    }
                    InstKind::WorkerSpawn {
                        entry,
                        args,
                        direct,
                    } => {
                        let entry = read(slots, *entry, inst.span)?.clone();
                        let args = args
                            .iter()
                            .map(|id| read(slots, *id, inst.span).cloned())
                            .collect::<Result<Vec<_>, _>>()?;
                        crate::worker::spawn(engine, &entry, args, *direct).map_err(|failure| {
                            RuntimeError::Unwind {
                                span: inst.span,
                                failure,
                            }
                        })?
                    }
                    InstKind::Fut { op, future } => {
                        let Value::Future(future) = read(slots, *future, inst.span)? else {
                            return Err(invalid_state(inst.span, "future operation on a non-future"));
                        };
                        let future = future.clone();
                        match op {
                            ir::FutOp::Cancel => {
                                sched::cancel(&future);
                                Value::Undef
                            }
                            ir::FutOp::Done => Value::Bool(future.done()),
                            ir::FutOp::Cancelled => Value::Bool(future.cancelled()),
                        }
                    }
                    InstKind::RegionEnter => {
                        Value::Region(Rc::new(okkama_runtime_core::heap::Region::new()))
                    }
                    InstKind::RegionAdopt { region, value } | InstKind::RegionTrack { region, value } => {
                        let Value::Region(region) = read(slots, *region, inst.span)? else {
                            return Err(invalid_state(inst.span, "expected region value"));
                        };
                        let value = read(slots, *value, inst.span)?;
                        if matches!(inst.kind, InstKind::RegionAdopt { .. }) {
                            region.adopt(value);
                        } else {
                            region.track(value);
                        }
                        Value::Undef
                    }
                    InstKind::RegionTransfer { from, to, value } => {
                        let (Value::Region(from), Value::Region(to)) =
                            (read(slots, *from, inst.span)?, read(slots, *to, inst.span)?)
                        else {
                            return Err(invalid_state(inst.span, "expected region values"));
                        };
                        from.transfer(to, read(slots, *value, inst.span)?);
                        Value::Undef
                    }
                    InstKind::ObjInit {
                        object,
                        field,
                        value,
                    } => {
                        state.object_init(
                            read(slots, *object, inst.span)?,
                            *field,
                            read(slots, *value, inst.span)?.clone(),
                            inst.span,
                        )?;
                        Value::Undef
                    }
                    InstKind::FnClosure { func, captures } => {
                        Value::Fn(Rc::new(okkama_runtime_core::heap::Closure {
                            function: func.0,
                            captures: captures
                                .iter()
                                .map(|id| read(slots, *id, inst.span).cloned())
                                .collect::<Result<Vec<_>, _>>()?,
                            worker: None,
                        }))
                    }
                    InstKind::WorkerBind {
                        action,
                        entry,
                        fields,
                    } => crate::worker::bind(
                        read(slots, *action, inst.span)?,
                        read(slots, *entry, inst.span)?.clone(),
                        fields
                            .iter()
                            .map(|id| read(slots, *id, inst.span).cloned())
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                    .map_err(|message| invalid_state(inst.span, message))?,
                    // Cap — сам владелец: поле вычисляется при каждом чтении.
                    InstKind::CapMake(owner) => read(slots, *owner, inst.span)?.clone(),
                    InstKind::AnyBox(value) => {
                        Value::Any(Rc::new(read(slots, *value, inst.span)?.clone()))
                    }
                    InstKind::MapNew(entries) => operations::map_new(
                        entries
                            .iter()
                            .map(|(key, value)| {
                                Ok((
                                    read(slots, *key, inst.span)?.clone(),
                                    read(slots, *value, inst.span)?.clone(),
                                ))
                            })
                            .collect::<Result<Vec<_>, RuntimeError>>()?,
                        inst.span,
                    )?,
                    InstKind::MapLen(map) => {
                        operations::map_len(read(slots, *map, inst.span)?, inst.span)?
                    }
                    InstKind::MapHas { map, key } => Value::Bool(
                        operations::map_lookup(
                            read(slots, *map, inst.span)?,
                            read(slots, *key, inst.span)?,
                            inst.span,
                        )?
                        .is_some(),
                    ),
                    InstKind::MapGet { map, key } => operations::map_get(
                        module,
                        read(slots, *map, inst.span)?,
                        read(slots, *key, inst.span)?,
                        inst.span,
                    )?,
                    InstKind::MapKeys(map) => {
                        operations::map_keys(read(slots, *map, inst.span)?, inst.span)?
                    }
                    InstKind::MapValues(map) => {
                        operations::map_values(read(slots, *map, inst.span)?, inst.span)?
                    }
                    InstKind::MapRetag(map) => read(slots, *map, inst.span)?.clone(),
                    InstKind::MapBuilderNew => okkama_runtime_core::heap::maps::new_builder(),
                    InstKind::MapBuilderPut {
                        builder,
                        key,
                        value,
                    } => operations::map_builder_put(
                        read(slots, *builder, inst.span)?,
                        read(slots, *key, inst.span)?,
                        read(slots, *value, inst.span)?.clone(),
                        inst.span,
                    )?,
                    InstKind::MapBuilderFinish(builder) => {
                        operations::map_builder_finish(read(slots, *builder, inst.span)?, inst.span)?
                    }
                    InstKind::BoolNot(value) => {
                        operations::bool_not(read(slots, *value, inst.span)?, inst.span)?
                    }
                    InstKind::BoolCompare { op, left, right } => operations::bool_compare(
                        *op,
                        read(slots, *left, inst.span)?,
                        read(slots, *right, inst.span)?,
                        inst.span,
                    )?,
                    other => {
                        return Err(unsupported(
                            inst.span,
                            format!("instruction {other:?} is not supported yet"),
                        ));
                    }
                })
            })();
            let value = match evaluated {
                Ok(value) => value,
                Err(error) => {
                    let raised = from_error(error)?;
                    propagate(module, &mut frames, raised, inst.span)?;
                    continue;
                }
            };
            if let Some(result) = inst.result {
                slots[result.0] = Some(value);
            }
            continue;
        }
        let span = block.term.span;
        // Lookup передаёт найденный элемент последним параметром блока.
        let mut payload = None;
        let target = match &block.term.kind {
            TermKind::Panic(message) => {
                let raised = Failure::new(FailureKind::Panic(
                    read(slots, *message, span)?.string(span)?.to_owned(),
                ));
                propagate(module, &mut frames, raised, span)?;
                continue;
            }
            TermKind::Trap { message, .. } => {
                let raised = Failure::new(FailureKind::Program(message.clone()));
                propagate(module, &mut frames, raised, span)?;
                continue;
            }
            TermKind::Raise(value) => {
                let raised = failure(read(slots, *value, span)?, span)?.clone();
                propagate(module, &mut frames, raised, span)?;
                continue;
            }
            TermKind::Throw(value) => {
                let value = read(slots, *value, span)?;
                if let Value::NativeFailure(failure) = value {
                    let raised = Failure::new(FailureKind::Native(failure.clone()));
                    propagate(module, &mut frames, raised, span)?;
                    continue;
                }
                let declaration = state.object_declaration(module, value, span)?;
                let object = value
                    .as_object()
                    .map_err(|e| invalid_state(span, e.message))?;
                let raised = if object.declaration == okkama_runtime_core::heap::CANCELLED {
                    Failure::new(FailureKind::Thrown {
                        object: okkama_runtime_core::heap::Ref(object.clone()),
                        name: "Cancelled".into(),
                        message: Some("Operation was cancelled".into()),
                    })
                } else {
                    Failure::new(FailureKind::Thrown {
                        object: okkama_runtime_core::heap::Ref(object.clone()),
                        name: crate::host::error_type_name(&module.objects[declaration].name),
                        message: None,
                    })
                };
                propagate(module, &mut frames, raised, span)?;
                continue;
            }
            TermKind::Catch {
                failure: value,
                ty,
                handler,
                next,
            } => {
                let raised = failure(read(slots, *value, span)?, span)?;
                if let FailureKind::Native(failure) = &raised.kind {
                    let matches = match ty {
                        ir::CatchType::Object(id) => module.objects[id.0].is_native_failure(),
                        ir::CatchType::Role(id) => module.roles[id.0].is_error(),
                    };
                    if matches {
                        payload = Some(Value::NativeFailure(failure.clone()));
                        handler
                    } else {
                        next
                    }
                } else if let FailureKind::Thrown { object, .. } = &raised.kind {
                    let value = Value::Object(object.0.clone());
                    let matches = match ty {
                        ir::CatchType::Object(id) => {
                            state.object_declaration(module, &value, span)? == id.0
                        }
                        ir::CatchType::Role(id) => {
                            state.role_witness(module, &value, *id, span)?.is_some()
                                || (is_runtime_cancelled(&value) && module.roles[id.0].is_error())
                        }
                    };
                    if matches {
                        payload = Some(value);
                        handler
                    } else {
                        next
                    }
                } else {
                    next
                }
            }
            TermKind::ArrayLookup {
                array,
                index,
                found,
                missing,
            } => {
                let index = read(slots, *index, span)?.int(span)?;
                match state.array_lookup(read(slots, *array, span)?, index, span)? {
                    Some(value) => {
                        payload = Some(value.clone());
                        found
                    }
                    None => missing,
                }
            }
            TermKind::MapLookup {
                map,
                key,
                found,
                missing,
            } => match operations::map_lookup(
                read(slots, *map, span)?,
                read(slots, *key, span)?,
                span,
            )? {
                Some(value) => {
                    payload = Some(value.clone());
                    found
                }
                None => missing,
            },
            TermKind::Ret(value) => {
                let returned = value.map(|id| read(slots, id, span).cloned()).transpose()?;
                let destination = frame.destination;
                frames.pop();
                if let Some(caller) = frames.last_mut() {
                    match (destination, returned) {
                        (Some(id), Some(value)) => caller.slots[id.0] = Some(value),
                        (None, None) => {}
                        _ => return Err(invalid_state(span, "call result and return disagree")),
                    }
                    continue;
                }
                return Ok(returned);
            }
            TermKind::Br(target) => target,
            TermKind::Cond {
                condition,
                then_target,
                else_target,
            } => {
                if read(slots, *condition, span)?.boolean(span)? {
                    then_target
                } else {
                    else_target
                }
            }
            other => {
                return Err(unsupported(
                    span,
                    format!("terminator {other:?} is not supported yet"),
                ));
            }
        };
        // Сначала снимок ВСЕХ аргументов, затем запись параметров.
        // На обратном ребре возможна перестановка: (a, b) <- (b, a).
        let mut arguments = target
            .args
            .iter()
            .map(|id| read(slots, *id, span).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        arguments.extend(payload);
        for (param, value) in function.blocks[target.block.0].params.iter().zip(arguments) {
            slots[param.0] = Some(value);
        }
        frame.block = target.block;
        frame.next_inst = 0;
    }
}

fn validate_native_signature(function: &ir::Function, span: Span) -> Result<(), RuntimeError> {
    use okkama_runtime_core::abi::{Native, Type};
    let symbol = function.extern_symbol().expect("native call");
    // Без provider проверять нечего: ошибку даёт сам вызов (`State::native`).
    let Some(native) = Native::resolve(symbol) else {
        return Ok(());
    };
    // Тип IR соответствует типу ABI; дескриптор — любой `native object`.
    fn fits(actual: &ir::Type, expected: &Type) -> bool {
        match (expected, actual) {
            (Type::Int, ir::Type::Int)
            | (Type::Float, ir::Type::Float)
            | (Type::Rat, ir::Type::Rat)
            | (Type::Bool, ir::Type::Bool)
            | (Type::Byte, ir::Type::Byte)
            | (Type::Char, ir::Type::Char)
            | (Type::Str, ir::Type::Str)
            | (Type::Any, ir::Type::Any)
            | (Type::Handle(_), ir::Type::Handle(_)) => true,
            (Type::Array(item), ir::Type::Array(actual)) => fits(actual, item),
            (Type::Map(key, value), ir::Type::Map(actual_key, actual_value)) => {
                fits(actual_key, key) && fits(actual_value, value)
            }
            _ => false,
        }
    }
    let result_fits = match &function.result {
        ir::ResultType::Value(actual) => fits(actual, native.result()),
        ir::ResultType::Void => *native.result() == Type::Void,
    };
    if function.params.len() != native.parameters().len()
        || function
            .params
            .iter()
            .zip(native.parameters())
            .any(|(param, expected)| !fits(&function.values[param.0], expected))
        || !result_fits
    {
        return Err(invalid_state(
            span,
            format!("native signature does not match ABI: {symbol}"),
        ));
    }
    Ok(())
}

/// Один динамический вызов. ValueId локальны: рекурсия получает новые слоты.
#[derive(Debug)]
struct Frame {
    function: ir::FuncId,
    block: ir::BlockId,
    next_inst: usize,
    slots: Vec<Option<Value>>,
    /// Куда записать результат в вызывающем кадре; None для void и корня.
    destination: Option<ir::ValueId>,
    /// Незавершённое закрытие региона инструкцией `region.leave`/`unwind`
    /// этого кадра: она исполняется заново после каждого `destroy`.
    closing: Option<PendingClose>,
    /// Кадр тела `destroy`: его отказ не раскручивает вызывающий кадр, а
    /// становится отказом закрытия региона.
    boundary: bool,
}

#[derive(Debug)]
struct PendingClose {
    closing: okkama_runtime_core::heap::Closing,
    failures: Vec<Rc<Failure>>,
}

impl Frame {
    fn new(
        module: &ir::Module,
        id: ir::FuncId,
        args: Vec<Value>,
        destination: Option<ir::ValueId>,
        span: Span,
    ) -> Result<Self, RuntimeError> {
        let function = &module.functions[id.0];
        if function.extern_symbol().is_some() || function.intrinsic().is_some() {
            return Err(unsupported(
                span,
                format!(
                    "async native and intrinsic calls are not supported yet: @{}",
                    function.symbol
                ),
            ));
        }
        if args.len() != function.params.len() {
            return Err(invalid_state(
                span,
                "call argument count does not match parameters",
            ));
        }
        let mut slots = vec![None; function.values.len()];
        for (param, value) in function.params.iter().zip(args) {
            slots[param.0] = Some(value);
        }
        Ok(Self {
            function: id,
            block: ir::BlockId(0),
            next_inst: 0,
            slots,
            destination,
            closing: None,
            boundary: false,
        })
    }
}

fn read(slots: &[Option<Value>], id: ir::ValueId, span: Span) -> Result<&Value, RuntimeError> {
    slots
        .get(id.0)
        .and_then(Option::as_ref)
        .ok_or_else(|| invalid_state(span, format!("value %{} is not initialized", id.0)))
}

fn failure(value: &Value, span: Span) -> Result<&Rc<Failure>, RuntimeError> {
    match value {
        Value::Failure(failure) => Ok(failure),
        _ => Err(invalid_state(span, "expected failure value")),
    }
}
fn is_runtime_cancelled(value: &Value) -> bool {
    matches!(value, Value::Object(object) if object.declaration == okkama_runtime_core::heap::CANCELLED)
}

/// Error-объекты отказа, его причин и вторичных отказов.
fn thrown_errors(failure: &Rc<Failure>) -> Vec<Value> {
    let mut errors = Vec::new();
    let mut pending = vec![failure.clone()];
    while let Some(link) = pending.pop() {
        if let FailureKind::Thrown { object, .. } = &link.kind {
            errors.push(Value::Object(object.0.clone()));
        }
        pending.extend(link.secondaries().into_iter().rev());
        if let Some(cause) = link.cause() {
            pending.push(cause);
        }
    }
    errors
}

fn from_error(error: RuntimeError) -> Result<Rc<Failure>, RuntimeError> {
    match error {
        RuntimeError::NativeFailure { failure, .. } => {
            Ok(Failure::new(FailureKind::Native(Rc::new(failure))))
        }
        RuntimeError::Panic { message, .. } => Ok(Failure::new(FailureKind::Panic(message))),
        RuntimeError::Trap { message, .. } => Ok(Failure::new(FailureKind::Program(message))),
        RuntimeError::Unwind { failure, .. } => Ok(failure),
        other => Err(other),
    }
}
fn propagate(
    module: &ir::Module,
    frames: &mut Vec<Frame>,
    failure: Rc<Failure>,
    span: Span,
) -> Result<(), RuntimeError> {
    while let Some(frame) = frames.last_mut() {
        let function = &module.functions[frame.function.0];
        if let Some(target) = &function.blocks[frame.block.0].unwind {
            let mut arguments = target
                .args
                .iter()
                .map(|id| read(&frame.slots, *id, span).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            arguments.push(Value::Failure(failure));
            for (param, value) in function.blocks[target.block.0].params.iter().zip(arguments) {
                frame.slots[param.0] = Some(value);
            }
            frame.block = target.block;
            frame.next_inst = 0;
            return Ok(());
        }
        let boundary = frame.boundary;
        frames.pop();
        if boundary {
            // Отказ тела destroy — отказ закрытия региона вызывающего кадра.
            let caller = frames
                .last_mut()
                .and_then(|caller| caller.closing.as_mut())
                .ok_or_else(|| invalid_state(span, "destroy returned outside region close"))?;
            caller.failures.push(failure);
            return Ok(());
        }
    }
    Err(failure_error(failure, span))
}

/// Отказ, дошедший до корня: простой — ошибкой своего вида, иначе раскрутка.
fn failure_error(failure: Rc<Failure>, span: Span) -> RuntimeError {
    if failure.secondary.borrow().is_empty()
        && failure.cause.borrow().is_none()
        && !matches!(failure.kind, FailureKind::Thrown { .. })
    {
        return match &failure.kind {
            FailureKind::Native(failure) => RuntimeError::NativeFailure {
                span,
                failure: (**failure).clone(),
            },
            FailureKind::Thrown { .. } => unreachable!("handled below"),
            FailureKind::Panic(message) => RuntimeError::Panic {
                span,
                message: message.clone(),
            },
            FailureKind::Program(message) => RuntimeError::Trap {
                span,
                message: message.clone(),
            },
        };
    }
    RuntimeError::Unwind { span, failure }
}

pub(crate) fn describe_failure(
    engine: &Rc<Engine>,
    failure: &Rc<Failure>,
    span: Span,
) -> Result<Rc<Failure>, RuntimeError> {
    let (module, state) = (&engine.module, &engine.state);
    let mut described = (**failure).clone();
    if let FailureKind::Thrown {
        object, message, ..
    } = &mut described.kind
    {
        if message.is_none() {
            let value = Value::Object(object.0.clone());
            let (role, declaration) = module
                .roles
                .iter()
                .enumerate()
                .find(|(_, role)| role.is_error())
                .ok_or_else(|| invalid_state(span, "Error role is missing"))?;
            let slot = declaration
                .methods
                .iter()
                .position(|method| method.name == "text")
                .ok_or_else(|| invalid_state(span, "Error.text is missing"))?;
            let function = state
                .role_witness(module, &value, ir::RoleId(role), span)?
                .and_then(|w| w.slots.get(slot))
                .copied()
                .ok_or_else(|| invalid_state(span, "Error.text witness is missing"))?;
            let text = execute_function(engine, function, vec![value])
                .map_err(|_| {
                    unsupported(
                        span,
                        "failure while formatting Error.text is not supported yet",
                    )
                })?
                .ok_or_else(|| invalid_state(span, "Error.text returned void"))?;
            *message = Some(text.string(span)?.to_owned());
        }
    }
    described.secondary = std::cell::RefCell::new(failure
        .secondaries()
        .iter()
        .map(|f| describe_failure(engine, f, span))
        .collect::<Result<_, _>>()?);
    // Causes are retained as records; diagnostics currently print primary and cleanup failures.
    Ok(Rc::new(described))
}
