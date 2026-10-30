//! Исполнитель кадров Sparrow: регистры, переходы, вызовы, отказы и регионы.
//! Семантика каждой инструкции — та же, что у соответствующей операции OIR
//! в исполнителе IR: значения и операции — общие (`okkama_runtime_core`).
use crate::{Error, OperationError};
use okkama_runtime_core::abi::{Native, NativeFailure};
use okkama_runtime_core::failure::{Failure, Kind, Site};
use okkama_runtime_core::heap::{ArrayHeap, Closing, ObjectHeap, Ref, Region};
use okkama_runtime_core::ops::{self, Fault};
use okkama_runtime_core::{IntValue, Value, sched};
use okkama_sparrow::{
    CatchKind, Cmp, Constant, Dst, Function, Instruction as I, Program, Register, RoleRef, Target, Type, TypeRef,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Остановка инструкции: отказ программы или ошибка исполнителя.
#[derive(Debug)]
pub(crate) enum Stop {
    Unwind(Rc<Failure>),
    Panic(String),
    Native(NativeFailure),
    Trap(String),
    Invalid(String),
    Unsupported(String),
    /// Остановка, чьё место уже известно (вложенное исполнение, машинный
    /// код): место сохраняется, вид определяет вложенная остановка.
    Located(Box<Located>),
}

impl From<Fault> for Stop {
    fn from(fault: Fault) -> Self {
        match fault {
            Fault::Trap(message) => Stop::Trap(message),
            Fault::Invalid(message) => Stop::Invalid(message),
            Fault::Unsupported(message) => Stop::Unsupported(message),
        }
    }
}

impl From<okkama_runtime_core::ValueError> for Stop {
    fn from(error: okkama_runtime_core::ValueError) -> Self {
        Stop::Invalid(error.message)
    }
}

/// Остановка с местом: функция и инструкция.
#[derive(Debug)]
pub(crate) struct Located {
    pub(crate) function: usize,
    pub(crate) pc: usize,
    pub(crate) stop: Stop,
}

pub(crate) fn invalid(message: impl Into<String>) -> Stop {
    Stop::Invalid(message.into())
}

/// Исполнитель одного запуска: программа и состояние natives.
#[derive(Debug)]
pub(crate) struct Engine {
    pub(crate) program: Program,
    native: RefCell<okkama_runtime_core::native::Context>,
    /// Ошибка исполнителя внутри задачи: завершает запуск после планировщика.
    fatal: RefCell<Option<Located>>,
    /// Подготовленные функции: создаются при первом вызове.
    prepared: Vec<std::cell::OnceCell<Prepared>>,
    /// Компилятор горячих функций в машинный код; `None` — выключен.
    pub(crate) jit: Option<Box<crate::jit::Jit>>,
    /// Отказ, дошедший до корня запуска (его стек вызовов — для диагностики).
    uncaught: RefCell<Option<Rc<Failure>>>,
}

/// Функция, подготовленная к исполнению: значения констант по номеру
/// инструкции (разбираются один раз, а не при каждом исполнении).
#[derive(Debug)]
pub(crate) struct Prepared {
    pub(crate) constants: Box<[Value]>,
}

impl Prepared {
    fn new(function: &Function) -> Result<Self, Stop> {
        let constants = function
            .code
            .iter()
            .map(|instruction| match instruction {
                I::Const { value, .. } => constant(value),
                _ => Ok(Value::Undef),
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { constants })
    }
}

pub(crate) fn new_engine(program: &Program) -> Rc<Engine> {
    let mut context = okkama_runtime_core::native::Context::default();
    context.objects = program
        .objects
        .iter()
        .map(|object| (object.name.clone(), object.fields.iter().map(|(name, _)| name.clone()).collect()))
        .collect();
    context.enums = program
        .enums
        .iter()
        .map(|decl| {
            let name = decl.name.rsplit("::").next().unwrap_or(&decl.name).to_string();
            (name, decl.cases.clone())
        })
        .collect();
    Rc::new_cyclic(|me| Engine {
        program: program.clone(),
        native: RefCell::new(context),
        fatal: RefCell::new(None),
        prepared: program.functions.iter().map(|_| std::cell::OnceCell::new()).collect(),
        jit: crate::jit::Jit::new(program, me.as_ptr()),
        uncaught: RefCell::new(None),
    })
}

impl Engine {
    pub(crate) fn set_arguments(&self, arguments: Vec<String>) {
        self.native.borrow_mut().arguments = arguments;
    }

    pub(crate) fn take_fatal(&self) -> Result<(), Located> {
        match self.fatal.borrow_mut().take() {
            Some(fatal) => Err(fatal),
            None => Ok(()),
        }
    }

    /// Подготовленная функция `index`.
    pub(crate) fn prepared(&self, index: usize) -> Result<&Prepared, Stop> {
        let cell = &self.prepared[index];
        if let Some(prepared) = cell.get() {
            return Ok(prepared);
        }
        let prepared = Prepared::new(&self.program.functions[index])?;
        Ok(cell.get_or_init(|| prepared))
    }

    pub(crate) fn case_name(&self, declaration: usize, case: usize) -> String {
        self.program.enums[declaration].cases[case].clone()
    }

    /// Объявление объекта; системный `Cancelled` рантайма — объявление
    /// `Cancelled` программы, если она его называет.
    pub(crate) fn object_declaration(&self, value: &Value) -> Result<usize, Stop> {
        let declaration = ObjectHeap.declaration(value)?;
        if declaration != okkama_runtime_core::heap::CANCELLED {
            return Ok(declaration);
        }
        Ok(self
            .program
            .objects
            .iter()
            .position(|object| object.builtin && object.name == "Cancelled")
            .unwrap_or(declaration))
    }

    pub(crate) fn role_witness(&self, value: &Value, role: RoleRef) -> Result<Option<&okkama_sparrow::Witness>, Stop> {
        let declaration = self.object_declaration(value)?;
        if declaration == okkama_runtime_core::heap::CANCELLED {
            return Ok(None);
        }
        let object = self
            .program
            .objects
            .get(declaration)
            .ok_or_else(|| invalid("object declaration is missing"))?;
        Ok(object.does.iter().find(|witness| witness.role == role))
    }

    fn ty(&self, ty: TypeRef) -> &Type {
        &self.program.types[ty.0 as usize]
    }

    fn native(&self, symbol: &str, args: &[Value]) -> Result<Value, Stop> {
        let Some(native) = Native::resolve(symbol) else {
            // Как JVM: native программы без provider — ошибка вызова.
            if !symbol.starts_with("runtime/") {
                return Err(Stop::Trap(format!("Native symbol \"{symbol}\" is not registered")));
            }
            return Err(Stop::Unsupported(format!("native provider is not implemented: {symbol}")));
        };
        okkama_runtime_core::native::execute(native, args, &mut ArrayHeap, &mut self.native.borrow_mut()).map_err(|error| match error {
            okkama_runtime_core::native::Error::InvalidState(message) => Stop::Invalid(message),
            okkama_runtime_core::native::Error::Unsupported(message) => Stop::Unsupported(message),
            okkama_runtime_core::native::Error::Trap(message) => Stop::Trap(message),
            okkama_runtime_core::native::Error::Failure(failure) => Stop::Native(failure),
        })
    }
}

/// Тип регистра соответствует типу ABI; дескриптор — любой `native object`.
fn fits(engine: &Engine, actual: TypeRef, expected: &okkama_runtime_core::abi::Type) -> bool {
    use okkama_runtime_core::abi::Type as A;
    match (expected, engine.ty(actual)) {
        (A::Int, Type::Int)
        | (A::Float, Type::Float)
        | (A::Rat, Type::Rat)
        | (A::Bool, Type::Bool)
        | (A::Byte, Type::Byte)
        | (A::Char, Type::Char)
        | (A::Str, Type::Str)
        | (A::Any, Type::Any)
        | (A::Handle(_), Type::Handle(_)) => true,
        (A::Array(item), Type::Array(actual)) => fits(engine, *actual, item),
        (A::Map(key, value), Type::Map(actual_key, actual_value)) => fits(engine, *actual_key, key) && fits(engine, *actual_value, value),
        _ => false,
    }
}

fn validate_native_signature(engine: &Engine, function: &Function, symbol: &str) -> Result<(), Stop> {
    let Some(native) = Native::resolve(symbol) else {
        return Ok(());
    };
    let result_fits = match function.result {
        Some(actual) => fits(engine, actual, native.result()),
        None => *native.result() == okkama_runtime_core::abi::Type::Void,
    };
    if function.params.len() != native.parameters().len()
        || function
            .params
            .iter()
            .zip(native.parameters())
            .any(|(param, expected)| !fits(engine, function.registers[param.0 as usize], expected))
        || !result_fits
    {
        return Err(invalid(format!("native signature does not match ABI: {symbol}")));
    }
    Ok(())
}

#[derive(Debug)]
struct PendingClose {
    closing: Closing,
    failures: Vec<Rc<Failure>>,
}

#[derive(Debug)]
struct Frame {
    function: usize,
    pc: usize,
    /// Регистры; ещё не записанный — `Undef` (верификатор доказывает запись
    /// до чтения).
    registers: Vec<Value>,
    /// Куда записать результат в вызывающем кадре; `None` у void и корня.
    destination: Option<Register>,
    /// Незавершённое закрытие региона инструкцией этого кадра: она
    /// исполняется заново после каждого `destroy`.
    closing: Option<PendingClose>,
    /// Кадр тела `destroy`: его отказ становится отказом закрытия региона.
    boundary: bool,
}

impl Frame {
    fn new(program: &Program, index: usize, args: Vec<Value>, destination: Option<Register>) -> Result<Self, Stop> {
        let function = &program.functions[index];
        if function.native.is_some() || function.intrinsic.is_some() {
            return Err(Stop::Unsupported(format!(
                "async native and intrinsic calls are not supported yet: @{}",
                function.symbol
            )));
        }
        if args.len() != function.params.len() {
            return Err(invalid("call argument count does not match parameters"));
        }
        let mut registers = vec![Value::Undef; function.registers.len()];
        for (param, value) in function.params.iter().zip(args) {
            registers[param.0 as usize] = value;
        }
        Ok(Self {
            function: index,
            pc: 0,
            registers,
            destination,
            closing: None,
            boundary: false,
        })
    }
}

pub(crate) fn read(registers: &[Value], register: Register) -> Result<&Value, Stop> {
    registers
        .get(register.0 as usize)
        .ok_or_else(|| invalid(format!("register r{} is out of range", register.0)))
}

pub(crate) fn failure(value: &Value) -> Result<&Rc<Failure>, Stop> {
    match value {
        Value::Failure(failure) => Ok(failure),
        _ => Err(invalid("expected failure value")),
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
        if let Kind::Thrown { object, .. } = &link.kind {
            errors.push(Value::Object(object.0.clone()));
        }
        pending.extend(link.secondaries().into_iter().rev());
        if let Some(cause) = link.cause() {
            pending.push(cause);
        }
    }
    errors
}

/// Остановка программы как отказ; ошибки исполнителя остаются собой.
pub(crate) fn into_failure(stop: Stop) -> Result<Rc<Failure>, Stop> {
    match stop {
        Stop::Native(failure) => Ok(Failure::new(Kind::Native(Rc::new(failure)))),
        Stop::Panic(message) => Ok(Failure::new(Kind::Panic(message))),
        Stop::Trap(message) => Ok(Failure::new(Kind::Program(message))),
        Stop::Unwind(failure) => Ok(failure),
        other => Err(other),
    }
}

/// Отказ, дошедший до корня: простой — остановкой своего вида, иначе раскрутка.
fn failure_stop(failure: Rc<Failure>) -> Stop {
    if failure.secondary.borrow().is_empty() && failure.cause.borrow().is_none() && !matches!(failure.kind, Kind::Thrown { .. }) {
        return match &failure.kind {
            Kind::Native(native) => Stop::Native((**native).clone()),
            Kind::Thrown { .. } => unreachable!("handled below"),
            Kind::Panic(message) => Stop::Panic(message.clone()),
            Kind::Program(message) => Stop::Trap(message.clone()),
        };
    }
    Stop::Unwind(failure)
}

/// Вызов async-функции: холодный Future над её телом и аргументами.
pub(crate) fn cold_call(engine: &Rc<Engine>, function: usize, args: Vec<Value>) -> sched::Future {
    let body_engine = engine.clone();
    let retained = args.clone();
    sched::cold(Box::new(move || run_body(&body_engine, function, args)), retained)
}

/// Тело задачи: исход вызова; ошибка исполнителя запоминается для запуска.
pub(crate) fn run_body(engine: &Rc<Engine>, function: usize, args: Vec<Value>) -> sched::Outcome {
    match invoke(engine, function, args) {
        Ok(value) => Ok(value.unwrap_or(Value::Undef)),
        Err(located) => match exit(located) {
            Exit::Raise(failure, _) => Err(failure),
            Exit::Fatal(fatal) => {
                let message = format!("{}", stop_error(&fatal.stop));
                engine.fatal.borrow_mut().get_or_insert(fatal);
                Err(Failure::new(Kind::Program(message)))
            }
        },
    }
}

/// Исход вложенного исполнения: отказ программы или ошибка исполнителя с местом.
#[derive(Debug)]
pub(crate) enum Exit {
    /// Отказ и место инструкции, где он возник (функция, номер).
    Raise(Rc<Failure>, (usize, usize)),
    Fatal(Located),
}

pub(crate) fn exit(located: Located) -> Exit {
    match located.stop {
        Stop::Located(inner) => exit(*inner),
        stop => match into_failure(stop) {
            Ok(failure) => Exit::Raise(failure, (located.function, located.pc)),
            Err(fatal) => Exit::Fatal(Located {
                function: located.function,
                pc: located.pc,
                stop: fatal,
            }),
        },
    }
}

/// Остановка инструкции `at` как исход вложенного исполнения.
pub(crate) fn stop_exit(stop: Stop, at: (usize, usize)) -> Exit {
    match stop {
        Stop::Located(located) => exit(*located),
        stop => match into_failure(stop) {
            Ok(failure) => Exit::Raise(failure, at),
            Err(stop) => Exit::Fatal(Located { function: at.0, pc: at.1, stop }),
        },
    }
}

/// Исполнить обычную функцию: машинным кодом, если он есть (или функция
/// стала горячей) и стек позволяет, иначе интерпретатором.
pub(crate) fn invoke(engine: &Rc<Engine>, index: usize, args: Vec<Value>) -> Result<Option<Value>, Located> {
    if let Some(jit) = &engine.jit
        && let Some(code) = jit.hot(engine, index)
        && crate::jit::stack_allows()
    {
        let function = &engine.program.functions[index];
        if args.len() != function.params.len() {
            return Err(Located { function: index, pc: 0, stop: invalid("call argument count does not match parameters") });
        }
        let mut registers = vec![Value::Undef; function.registers.len()];
        for (param, value) in function.params.iter().zip(args) {
            registers[param.0 as usize] = value;
        }
        return match jit.enter(code, &mut registers, 0, function.result.is_some()) {
            Ok(value) => Ok(value),
            Err(Exit::Raise(failure, at)) => Err(Located { function: at.0, pc: at.1, stop: Stop::Unwind(failure) }),
            Err(Exit::Fatal(located)) => Err(Located { function: located.function, pc: located.pc, stop: Stop::Located(Box::new(located)) }),
        };
    }
    execute_function(engine, index, args)
}

/// Вызов из машинного кода: intrinsic, холодный Future, native или функция.
/// `at` — место вызывающей инструкции (функция, номер) для ошибок исполнителя.
pub(crate) fn call_now(engine: &Rc<Engine>, index: usize, args: Vec<Value>, at: (usize, usize)) -> Result<Option<Value>, Exit> {
    let program = &engine.program;
    let function = &program.functions[index];
    let fatal = |stop: Stop| stop_exit(stop, at);
    if let Some(intrinsic) = &function.intrinsic {
        let host: okkama_runtime_core::intrinsics::HostRef = Rc::new(crate::host::VmHost(engine.clone()));
        let objects: Vec<usize> = intrinsic.objects.iter().map(|object| object.0 as usize).collect();
        let helpers: Vec<usize> = intrinsic.helpers.iter().map(|helper| helper.0 as usize).collect();
        return okkama_runtime_core::intrinsics::call(&host, &intrinsic.name, &objects, &helpers, args)
            .map(Some)
            .map_err(|failure| Exit::Raise(failure, at));
    }
    if function.is_async && function.native.is_none() {
        return Ok(Some(Value::Future(cold_call(engine, index, args))));
    }
    if let Some(symbol) = &function.native {
        validate_native_signature(engine, function, symbol).map_err(fatal)?;
        return engine.native(symbol, &args).map(Some).map_err(fatal);
    }
    if function.params.len() != args.len() {
        return Err(fatal(invalid("call argument count does not match parameters")));
    }
    invoke(engine, index, args).map_err(exit)
}

/// `region.leave`/`region.unwind` целиком: `destroy` исполняются вложенно.
pub(crate) fn close_now(engine: &Rc<Engine>, registers: &mut [Value], instruction: &I, at: (usize, usize)) -> Result<(), Exit> {
    let program = &engine.program;
    let here = |stop: Stop| stop_exit(stop, at);
    let (region, keep_or_failure) = match instruction {
        I::RegionLeave { region, keep } => (*region, keep.map(|keep| (false, keep))),
        I::RegionUnwind { region, failure, .. } => (*region, Some((true, *failure))),
        _ => unreachable!("region close"),
    };
    let Value::Region(region) = read(registers, region).map_err(here)? else {
        return Err(here(invalid("expected region value")));
    };
    let region: Rc<Region> = region.clone();
    let keep = match keep_or_failure {
        Some((false, keep)) => vec![read(registers, keep).map_err(here)?.clone()],
        Some((true, id)) => thrown_errors(failure(read(registers, id).map_err(here)?).map_err(here)?),
        None => Vec::new(),
    };
    let mut closing = region.begin_close(&keep);
    let mut failures = Vec::new();
    while let Some(object) = region.next_destroyed(&mut closing) {
        if let Some(body) = program.objects.get(object.declaration).and_then(|object| object.destroy) {
            match invoke(engine, body.0 as usize, vec![Value::Object(object)]).map_err(exit) {
                Ok(_) => {}
                Err(Exit::Raise(failure, _)) => failures.push(failure),
                Err(fatal) => return Err(fatal),
            }
        }
    }
    match instruction {
        I::RegionUnwind { dst, failure: id, .. } => {
            let mut active = failure(read(registers, *id).map_err(here)?).map_err(here)?.clone();
            for cleanup in failures {
                active = active.with_secondary(cleanup);
            }
            registers[dst.0.0 as usize] = Value::Failure(active);
            Ok(())
        }
        _ => {
            let mut failures = failures.into_iter();
            match failures.next() {
                Some(mut primary) => {
                    for cleanup in failures {
                        primary = primary.with_secondary(cleanup);
                    }
                    Err(Exit::Raise(primary, at))
                }
                None => Ok(()),
            }
        }
    }
}

fn stop_error(stop: &Stop) -> OperationError {
    match stop {
        Stop::Unwind(failure) => OperationError::Unwind(failure.clone()),
        Stop::Panic(message) => OperationError::Panic { message: message.clone() },
        Stop::Native(failure) => OperationError::NativeFailure(failure.clone()),
        Stop::Trap(message) => OperationError::Trap { message: message.clone() },
        Stop::Invalid(message) => OperationError::InvalidState { message: message.clone() },
        Stop::Unsupported(message) => OperationError::Unsupported { message: message.clone() },
        Stop::Located(located) => stop_error(&located.stop),
    }
}

/// Распространить отказ: ближайший обработчик или отказ закрытия региона.
fn propagate(program: &Program, frames: &mut Vec<Frame>, failure: Rc<Failure>) -> Result<(), Stop> {
    while let Some(frame) = frames.last_mut() {
        let function = &program.functions[frame.function];
        let failing = frame.pc.saturating_sub(1) as u32;
        failure.trace.record(
            frame.registers.as_ptr() as usize,
            Site {
                function: frame.function as u32,
                pc: failing,
            },
        );
        if let Some(handler) = function.handler_at(failing) {
            let values = handler
                .target
                .moves
                .iter()
                .map(|(_, source)| read(&frame.registers, *source).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            for ((dst, _), value) in handler.target.moves.iter().zip(values) {
                frame.registers[dst.0 as usize] = value;
            }
            frame.registers[handler.failure.0 as usize] = Value::Failure(failure);
            frame.pc = handler.target.pc as usize;
            return Ok(());
        }
        let boundary = frame.boundary;
        frames.pop();
        if boundary {
            let caller = frames
                .last_mut()
                .and_then(|caller| caller.closing.as_mut())
                .ok_or_else(|| invalid("destroy returned outside region close"))?;
            caller.failures.push(failure);
            return Ok(());
        }
    }
    // Корень вложенного исполнения: отказ — тот же объект (идентичность
    // важна для `caused`); вид остановки корня задаёт `root_stop`.
    Err(Stop::Unwind(failure))
}

/// Остановка, дошедшая до корня запуска: простой отказ — своим видом.
fn root_stop(stop: Stop) -> Stop {
    match stop {
        Stop::Unwind(failure) => failure_stop(failure),
        other => other,
    }
}

/// Перейти: снимок всех источников, затем запись назначений и значения ребра.
fn transfer(frame: &mut Frame, target: &Target, payload: Option<(Register, Value)>) -> Result<(), Stop> {
    transfer_registers(&mut frame.registers, target, payload)?;
    frame.pc = target.pc as usize;
    Ok(())
}

/// Записи перехода над регистрами: снимок источников, назначения, значение ребра.
pub(crate) fn transfer_registers(registers: &mut [Value], target: &Target, payload: Option<(Register, Value)>) -> Result<(), Stop> {
    match target.moves.as_slice() {
        [] => {}
        // Одна запись: снимок не нужен.
        [(dst, source)] => {
            let value = read(registers, *source)?.clone();
            registers[dst.0 as usize] = value;
        }
        moves => {
            let values = moves
                .iter()
                .map(|(_, source)| read(registers, *source).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            for ((dst, _), value) in moves.iter().zip(values) {
                registers[dst.0 as usize] = value;
            }
        }
    }
    if let Some((register, value)) = payload {
        registers[register.0 as usize] = value;
    }
    Ok(())
}

/// Шаг инструкции после её вычисления.
pub(crate) enum Step<'a> {
    /// Записать значение (если есть назначение).
    Write(Option<Dst>, Value),
    Call { function: usize, args: Vec<Value>, dst: Option<Dst> },
    Transfer(&'a Target, Option<(Register, Value)>),
    Return(Option<Value>),
    Raise(Rc<Failure>),
}

fn scalar(ty: &Type) -> ops::Scalar {
    match ty {
        Type::Int => ops::Scalar::Int,
        Type::Float => ops::Scalar::Float,
        Type::Rat => ops::Scalar::Rat,
        Type::Str => ops::Scalar::Str,
        _ => ops::Scalar::Other,
    }
}

fn cmp(op: Cmp) -> ops::Cmp {
    match op {
        Cmp::Eq => ops::Cmp::Eq,
        Cmp::Ne => ops::Cmp::Ne,
        Cmp::Lt => ops::Cmp::Lt,
        Cmp::Le => ops::Cmp::Le,
        Cmp::Gt => ops::Cmp::Gt,
        Cmp::Ge => ops::Cmp::Ge,
    }
}

fn int(value: &Value) -> Result<&IntValue, Stop> {
    Ok(value.as_int()?)
}

fn string(value: &Value) -> Result<&str, Stop> {
    Ok(value.as_string()?)
}

fn constant(value: &Constant) -> Result<Value, Stop> {
    Ok(match value {
        Constant::Int(text) => Value::Int(IntValue::parse(text).ok_or_else(|| invalid("invalid integer constant"))?),
        Constant::Bool(value) => Value::Bool(*value),
        Constant::Byte(value) => Value::Byte(*value),
        Constant::Float(bits) => ops::float_constant(*bits)?,
        Constant::Char(value) => Value::Char(*value),
        Constant::Str(text) => Value::Str(Rc::from(text.as_str())),
    })
}

/// Закрытие региона инструкцией `region.leave`/`region.unwind`: вызывает
/// `destroy` его идентичностей по одному (кадр-граница) и после последнего
/// пишет результат или распространяет отказ.
fn close_region(engine: &Rc<Engine>, frames: &mut Vec<Frame>, pc: usize, instruction: &I) -> Result<(), Stop> {
    let program = &engine.program;
    let frame = frames.last_mut().expect("frame");
    let (region, keep_or_failure) = match instruction {
        I::RegionLeave { region, keep } => (*region, keep.map(|keep| (false, keep))),
        I::RegionUnwind { region, failure, .. } => (*region, Some((true, *failure))),
        _ => unreachable!("region close"),
    };
    let Value::Region(region) = read(&frame.registers, region)? else {
        return Err(invalid("expected region value"));
    };
    let region: Rc<Region> = region.clone();
    if frame.closing.is_none() {
        let keep = match keep_or_failure {
            Some((false, keep)) => vec![read(&frame.registers, keep)?.clone()],
            // Error-объекты отказа освобождаются (`Region.unwind`).
            Some((true, id)) => thrown_errors(failure(read(&frame.registers, id)?)?),
            None => Vec::new(),
        };
        frame.closing = Some(PendingClose {
            closing: region.begin_close(&keep),
            failures: Vec::new(),
        });
    }
    let pending = frame.closing.as_mut().expect("closing started above");
    let mut destroy = None;
    while let Some(object) = region.next_destroyed(&mut pending.closing) {
        if let Some(body) = program.objects.get(object.declaration).and_then(|object| object.destroy) {
            destroy = Some((body, object));
            break;
        }
    }
    if let Some((body, object)) = destroy {
        // Инструкция исполнится заново, когда `destroy` завершится.
        frame.pc = pc;
        let mut callee = Frame::new(program, body.0 as usize, vec![Value::Object(object)], None)?;
        callee.boundary = true;
        frames.push(callee);
        return Ok(());
    }
    let failures = frame.closing.take().expect("closing started above").failures;
    match instruction {
        I::RegionUnwind { dst, failure: id, .. } => {
            // Отказы destroy на пути отказа — вторичные к нему.
            let mut active = failure(read(&frame.registers, *id)?)?.clone();
            for cleanup in failures {
                active = active.with_secondary(cleanup);
            }
            frame.registers[dst.0.0 as usize] = Value::Failure(active);
        }
        _ => {
            // Первый отказ destroy — основной, остальные — вторичные.
            let mut failures = failures.into_iter();
            if let Some(mut primary) = failures.next() {
                for cleanup in failures {
                    primary = primary.with_secondary(cleanup);
                }
                propagate(program, frames, primary)?;
            }
        }
    }
    Ok(())
}

/// Вызов функции: intrinsic, холодный Future, native или новый кадр.
fn call(engine: &Rc<Engine>, frames: &mut Vec<Frame>, index: usize, args: Vec<Value>, dst: Option<Dst>) -> Result<(), Stop> {
    let program = &engine.program;
    let function = &program.functions[index];
    let write = |frames: &mut Vec<Frame>, value: Value| {
        if let Some(dst) = dst {
            frames.last_mut().expect("frame").registers[dst.0.0 as usize] = value;
        }
    };
    if let Some(intrinsic) = &function.intrinsic {
        let host: okkama_runtime_core::intrinsics::HostRef = Rc::new(crate::host::VmHost(engine.clone()));
        let objects: Vec<usize> = intrinsic.objects.iter().map(|object| object.0 as usize).collect();
        let helpers: Vec<usize> = intrinsic.helpers.iter().map(|helper| helper.0 as usize).collect();
        match okkama_runtime_core::intrinsics::call(&host, &intrinsic.name, &objects, &helpers, args) {
            Ok(value) => write(frames, value),
            Err(raised) => propagate(program, frames, raised)?,
        }
        return Ok(());
    }
    if function.is_async && function.native.is_none() {
        let future = cold_call(engine, index, args);
        write(frames, Value::Future(future));
        return Ok(());
    }
    if let Some(symbol) = &function.native {
        validate_native_signature(engine, function, symbol)?;
        match engine.native(symbol, &args) {
            Ok(value) => write(frames, value),
            Err(stop) => {
                let raised = into_failure(stop)?;
                propagate(program, frames, raised)?;
            }
        }
        return Ok(());
    }
    if let Some(jit) = &engine.jit
        && let Some(code) = jit.hot(engine, index)
        && crate::jit::stack_allows()
    {
        let mut frame = Frame::new(program, index, args, None)?;
        match jit.enter(code, &mut frame.registers, 0, function.result.is_some()) {
            Ok(returned) => match (dst, returned) {
                (Some(_), Some(value)) => write(frames, value),
                (None, None) => {}
                _ => return Err(invalid("call result and return disagree")),
            },
            // Отказ, не принятый ни одним кадром, сообщается с местом возникновения.
            Err(Exit::Raise(failure, at)) => propagate(program, frames, failure).map_err(|stop| match stop {
                Stop::Unwind(failure) => Stop::Located(Box::new(Located { function: at.0, pc: at.1, stop: Stop::Unwind(failure) })),
                other => other,
            })?,
            Err(Exit::Fatal(located)) => return Err(Stop::Located(Box::new(located))),
        }
        return Ok(());
    }
    frames.push(Frame::new(program, index, args, dst.map(|dst| dst.0))?);
    Ok(())
}

/// Тип значения непередаваемого в воркер (`typeName` JVM).
pub(crate) fn type_name(program: &Program, value: &Value) -> String {
    match value {
        Value::Rat(_) => "rat".into(),
        Value::Enum { declaration, .. } => program.enums.get(*declaration).map_or_else(|| "enum".into(), |e| e.name.clone()),
        Value::Future(_) => "Future".into(),
        Value::Fn(_) => "action".into(),
        Value::Any(_) => "any".into(),
        Value::Object(object) => program
            .objects
            .get(object.declaration)
            .map_or_else(|| "object".into(), |declaration| declaration.name.rsplit(['.', '$']).next().unwrap_or("").to_string()),
        Value::Handle(_) => "NativeHandle".into(),
        Value::Region(_) => "Region".into(),
        Value::Range(_) => "Range".into(),
        _ => "value".into(),
    }
}

/// Вычислить инструкцию над регистрами текущего кадра.
pub(crate) fn evaluate<'a>(
    engine: &Rc<Engine>,
    function: &'a Function,
    constants: &[Value],
    pc: usize,
    registers: &[Value],
    instruction: &'a I,
) -> Result<Step<'a>, Stop> {
    let program = &engine.program;
    let r = |register: Register| read(registers, register);
    let write = |dst: Dst, value: Value| Ok(Step::Write(Some(dst), value));
    let int_op = |op: ops::IntOp, dst: Dst, left: Register, right: Register| -> Result<Step<'a>, Stop> {
        Ok(Step::Write(Some(dst), ops::int_binary(op, r(left)?, r(right)?)?))
    };
    let byte_op = |op: ops::IntOp, dst: Dst, left: Register, right: Register| -> Result<Step<'a>, Stop> {
        Ok(Step::Write(Some(dst), ops::byte_binary(op, r(left)?, r(right)?)?))
    };
    let float2 = |op: ops::FloatOp, dst: Dst, left: Register, right: Register| -> Result<Step<'a>, Stop> {
        Ok(Step::Write(Some(dst), ops::float_op(op, &[r(left)?, r(right)?])?))
    };
    let float1 = |op: ops::FloatOp, dst: Dst, operand: Register| -> Result<Step<'a>, Stop> {
        Ok(Step::Write(Some(dst), ops::float_op(op, &[r(operand)?])?))
    };
    let rat2 = |op: ops::RatOp, dst: Dst, left: Register, right: Register| -> Result<Step<'a>, Stop> {
        Ok(Step::Write(Some(dst), ops::rat_op(op, &[r(left)?, r(right)?])?))
    };
    let rat1 = |op: ops::RatOp, dst: Dst, operand: Register| -> Result<Step<'a>, Stop> {
        Ok(Step::Write(Some(dst), ops::rat_op(op, &[r(operand)?])?))
    };
    let string_op = |op: ops::StrOp, dst: Dst, args: &[Register]| -> Result<Step<'a>, Stop> {
        let values = args.iter().map(|register| r(*register)).collect::<Result<Vec<_>, _>>()?;
        Ok(Step::Write(Some(dst), ops::string(op, &values)?))
    };
    let values = |list: &[Register]| list.iter().map(|register| r(*register).cloned()).collect::<Result<Vec<_>, Stop>>();
    use ops::{FloatOp as F, IntOp as N, RatOp as Q, StrOp as S};
    match instruction {
        I::Const { dst, .. } => write(*dst, constants[pc].clone()),
        I::Move { dst, source } => write(*dst, r(*source)?.clone()),
        I::Undef { dst } => write(*dst, Value::Undef),
        I::IntAdd { dst, left, right } => int_op(N::Add, *dst, *left, *right),
        I::IntSub { dst, left, right } => int_op(N::Sub, *dst, *left, *right),
        I::IntMul { dst, left, right } => int_op(N::Mul, *dst, *left, *right),
        I::IntMod { dst, left, right } => int_op(N::Mod, *dst, *left, *right),
        I::IntAnd { dst, left, right } => int_op(N::And, *dst, *left, *right),
        I::IntOr { dst, left, right } => int_op(N::Or, *dst, *left, *right),
        I::IntXor { dst, left, right } => int_op(N::Xor, *dst, *left, *right),
        I::IntShl { dst, left, right } => int_op(N::Shl, *dst, *left, *right),
        I::IntShr { dst, left, right } => int_op(N::Shr, *dst, *left, *right),
        I::IntUshr { dst, left, right } => int_op(N::Ushr, *dst, *left, *right),
        I::IntNeg { dst, operand } => write(*dst, ops::int_unary(ops::IntUnOp::Neg, r(*operand)?)?),
        I::IntNot { dst, operand } => write(*dst, ops::int_unary(ops::IntUnOp::Not, r(*operand)?)?),
        I::IntAbs { dst, operand } => write(*dst, ops::int_abs(r(*operand)?)?),
        I::Sign { dst, operand } => write(*dst, ops::sign(r(*operand)?)?),
        I::IntCmp { op, dst, left, right } => write(*dst, ops::int_compare(cmp(*op), r(*left)?, r(*right)?)?),
        I::ByteFrom { dst, source, context } => write(*dst, ops::byte_from(r(*source)?, context.as_deref())?),
        I::Convert { dst, source } => {
            let target = engine.ty(function.registers[dst.0.0 as usize]);
            match ops::convert(r(*source)?, scalar(target)) {
                Ok(value) => write(*dst, value),
                Err(Fault::Unsupported(_)) => Err(Stop::Unsupported(format!("conversion to {target:?} is not supported yet"))),
                Err(other) => Err(other.into()),
            }
        }
        I::Bits { op, byte, dst, value, count } => {
            let code = okkama_runtime_core::integer::BitOp::from_code(op.code()).ok_or_else(|| invalid("unknown bit operation"))?;
            let count = count.map(|count| r(count)).transpose()?;
            write(*dst, ops::bits(code, op.counts(), *byte, r(*value)?, count)?)
        }
        I::ByteAnd { dst, left, right } => byte_op(N::And, *dst, *left, *right),
        I::ByteOr { dst, left, right } => byte_op(N::Or, *dst, *left, *right),
        I::ByteXor { dst, left, right } => byte_op(N::Xor, *dst, *left, *right),
        I::ByteShl { dst, left, right } => byte_op(N::Shl, *dst, *left, *right),
        I::ByteShr { dst, left, right } => byte_op(N::Shr, *dst, *left, *right),
        I::ByteUshr { dst, left, right } => byte_op(N::Ushr, *dst, *left, *right),
        I::ByteNot { dst, operand } => write(*dst, ops::byte_not(r(*operand)?)?),
        I::ByteCmp { op, dst, left, right } | I::CharCmp { op, dst, left, right } => {
            write(*dst, ops::scalar_compare(cmp(*op), r(*left)?, r(*right)?)?)
        }
        I::BoolNot { dst, operand } => write(*dst, ops::bool_not(r(*operand)?)?),
        I::BoolCmp { op, dst, left, right } => write(*dst, ops::bool_compare(cmp(*op), r(*left)?, r(*right)?)?),
        I::FloatAdd { dst, left, right } => float2(F::Add, *dst, *left, *right),
        I::FloatSub { dst, left, right } => float2(F::Sub, *dst, *left, *right),
        I::FloatMul { dst, left, right } => float2(F::Mul, *dst, *left, *right),
        I::FloatDiv { dst, left, right } => float2(F::Div, *dst, *left, *right),
        I::FloatMod { dst, left, right } => float2(F::Mod, *dst, *left, *right),
        I::FloatPow { dst, left, right } => float2(F::Pow, *dst, *left, *right),
        I::FloatNeg { dst, operand } => float1(F::Neg, *dst, *operand),
        I::FloatAbs { dst, operand } => float1(F::Abs, *dst, *operand),
        I::FloatSqrt { dst, operand } => float1(F::Sqrt, *dst, *operand),
        I::FloatCmp { op, dst, left, right } => write(*dst, ops::float_compare(cmp(*op), r(*left)?, r(*right)?)?),
        I::RatNew { dst, left, right } => rat2(Q::New, *dst, *left, *right),
        I::RatAdd { dst, left, right } => rat2(Q::Add, *dst, *left, *right),
        I::RatSub { dst, left, right } => rat2(Q::Sub, *dst, *left, *right),
        I::RatMul { dst, left, right } => rat2(Q::Mul, *dst, *left, *right),
        I::RatDiv { dst, left, right } => rat2(Q::Div, *dst, *left, *right),
        I::RatMod { dst, left, right } => rat2(Q::Mod, *dst, *left, *right),
        I::RatNeg { dst, operand } => rat1(Q::Neg, *dst, *operand),
        I::RatAbs { dst, operand } => rat1(Q::Abs, *dst, *operand),
        I::RatCmp { op, dst, left, right } => write(*dst, ops::rat_compare(cmp(*op), r(*left)?, r(*right)?)?),
        I::RatPart { dst, value, denominator } => write(*dst, ops::rat_part(r(*value)?, *denominator)?),
        I::StrConcat { dst, left, right } => write(*dst, ops::concat(r(*left)?, r(*right)?)?),
        I::StrCmp { op, dst, left, right } => write(*dst, ops::string_compare(cmp(*op), r(*left)?, r(*right)?)?),
        I::StrLen { dst, text } => string_op(S::Len, *dst, &[*text]),
        I::StrSlice { dst, text, start, end } => string_op(S::Slice, *dst, &[*text, *start, *end]),
        I::StrSplit { dst, text, separator } => string_op(S::Split, *dst, &[*text, *separator]),
        I::StrReplace { dst, text, pattern, replacement } => string_op(S::Replace, *dst, &[*text, *pattern, *replacement]),
        I::StrReplaceFirst { dst, text, pattern, replacement } => string_op(S::ReplaceFirst, *dst, &[*text, *pattern, *replacement]),
        I::StrTrim { dst, text } => string_op(S::Trim, *dst, &[*text]),
        I::StrChomp { dst, text } => string_op(S::Chomp, *dst, &[*text]),
        I::StrUtf8 { dst, text } => string_op(S::Utf8, *dst, &[*text]),
        I::ValCmp { op, dst, left, right } => {
            let equal = ops::equal(r(*left)?, r(*right)?)?;
            write(*dst, Value::Bool(if *op == Cmp::Eq { equal } else { !equal }))
        }
        I::EnumCase { dst, ty, case } => write(*dst, Value::Enum { declaration: ty.0 as usize, case: *case as usize }),
        I::EnumCmp { op, dst, left, right } => {
            let equal = r(*left)?.enum_eq(r(*right)?)?;
            write(*dst, Value::Bool(if *op == Cmp::Eq { equal } else { !equal }))
        }
        I::AnyBox { dst, source } => write(*dst, Value::Any(Rc::new(r(*source)?.clone()))),
        I::RangeNew { dst, start, end, inclusive } => write(*dst, ops::range_new(int(r(*start)?)?, int(r(*end)?)?, *inclusive)?),
        I::RangePart { dst, range, end } => write(*dst, ops::range_part(r(*range)?, *end)?),
        I::ObjNew { dst, object, fields } => {
            let declaration = &program.objects[object.0 as usize];
            if declaration.builtin {
                return Err(invalid("runtime objects are created by the runtime"));
            }
            write(*dst, ObjectHeap.allocate(object.0 as usize, values(fields)?))
        }
        I::ObjInit { object, field, value } => {
            ObjectHeap.init(r(*object)?, *field as usize, r(*value)?.clone())?;
            Ok(Step::Write(None, Value::Undef))
        }
        I::ObjGet { dst, object, field } => write(*dst, ops::object_get(r(*object)?, *field as usize)?),
        I::ObjNext { dst, object, updates } => {
            let updates = updates
                .iter()
                .map(|(field, value)| Ok((*field as usize, r(*value)?.clone())))
                .collect::<Result<Vec<_>, Stop>>()?;
            write(*dst, ObjectHeap.successor(r(*object)?, updates)?)
        }
        I::RoleWrap { dst, source, role } | I::RoleNarrow { dst, source, role, .. } => {
            let value = r(*source)?;
            let narrowing = match instruction {
                I::RoleNarrow { message, .. } => Some(message),
                _ => None,
            };
            if matches!(value, Value::NativeFailure(_)) {
                if program.roles[role.0 as usize].is_error() {
                    return write(*dst, value.clone());
                }
                return Err(match narrowing {
                    Some(message) => Stop::Trap(message.clone()),
                    None => invalid("NativeFailure only implements Error"),
                });
            }
            if engine.role_witness(value, *role)?.is_none() {
                return Err(match narrowing {
                    Some(message) => Stop::Trap(message.clone()),
                    None => invalid("role witness is missing"),
                });
            }
            write(*dst, value.clone())
        }
        I::ArrayNew { dst, elements } => write(*dst, ArrayHeap.allocate(values(elements)?)),
        I::ArrayLen { dst, array } => write(*dst, ops::array_len(r(*array)?)?),
        I::ArrayGet { dst, array, index } => write(*dst, ops::array_get(r(*array)?, int(r(*index)?)?)?),
        I::ArraySlice { dst, array, start, end } => write(*dst, ops::array_slice(r(*array)?, int(r(*start)?)?, int(r(*end)?)?)?),
        I::ArrayJoin { dst, array, separator } => write(*dst, ops::array_join(r(*array)?, string(r(*separator)?)?)?),
        I::BuilderNew { dst } => write(*dst, ArrayHeap.builder_new()),
        I::BuilderPush { dst, builder, value } => write(*dst, ArrayHeap.builder_push(r(*builder)?, r(*value)?.clone())?),
        I::BuilderAppend { dst, builder, array } => write(*dst, ArrayHeap.builder_append(r(*builder)?, r(*array)?)?),
        I::BuilderRange { dst, builder, start, end, inclusive } => {
            write(*dst, ops::builder_range(r(*builder)?, int(r(*start)?)?, int(r(*end)?)?, *inclusive)?)
        }
        I::BuilderFinish { dst, builder } => write(*dst, ArrayHeap.builder_finish(r(*builder)?)?),
        I::MapNew { dst, entries } => {
            let entries = entries
                .iter()
                .map(|(key, value)| Ok((r(*key)?.clone(), r(*value)?.clone())))
                .collect::<Result<Vec<_>, Stop>>()?;
            write(*dst, ops::map_new(entries)?)
        }
        I::MapLen { dst, map } => write(*dst, ops::map_len(r(*map)?)?),
        I::MapHas { dst, map, key } => write(*dst, Value::Bool(ops::map_lookup(r(*map)?, r(*key)?)?.is_some())),
        I::MapGet { dst, map, key } => write(*dst, ops::map_get(r(*map)?, r(*key)?, &|declaration, case| engine.case_name(declaration, case))?),
        I::MapKeys { dst, map } => write(*dst, ops::map_keys(r(*map)?)?),
        I::MapValues { dst, map } => write(*dst, ops::map_values(r(*map)?)?),
        I::MapBuilderNew { dst } => write(*dst, okkama_runtime_core::heap::maps::new_builder()),
        I::MapBuilderPut { dst, builder, key, value } => write(*dst, ops::map_builder_put(r(*builder)?, r(*key)?, r(*value)?.clone())?),
        I::MapBuilderFinish { dst, builder } => write(*dst, ops::map_builder_finish(r(*builder)?)?),
        I::Call { dst, function, args } => Ok(Step::Call {
            function: function.0 as usize,
            args: values(args)?,
            dst: *dst,
        }),
        I::Closure { dst, function, captures } => write(
            *dst,
            Value::Fn(Rc::new(okkama_runtime_core::heap::Closure {
                function: function.0 as usize,
                captures: values(captures)?,
                worker: None,
            })),
        ),
        // Замыкание: функция над захватами, затем аргументами.
        I::CallFn { dst, callee, args } => {
            let Value::Fn(closure) = r(*callee)? else {
                return Err(invalid("call.fn of a non-function value"));
            };
            let mut arguments = closure.captures.clone();
            arguments.extend(values(args)?);
            Ok(Step::Call {
                function: closure.function,
                args: arguments,
                dst: *dst,
            })
        }
        // Поле cap вычисляется заново над владельцем (§8.5).
        I::CallCap { dst, cap, ty, field } => {
            let function = program.caps[ty.0 as usize].fields[*field as usize].1;
            Ok(Step::Call {
                function: function.0 as usize,
                args: vec![r(*cap)?.clone()],
                dst: Some(*dst),
            })
        }
        I::CallRole { dst, receiver, role, slot, args } => {
            let receiver_value = r(*receiver)?;
            let declaration = &program.roles[role.0 as usize];
            if let Value::NativeFailure(failure) = receiver_value {
                if !declaration.is_error() || declaration.methods.get(*slot as usize).is_none_or(|m| m.name != "text") || !args.is_empty() {
                    return Err(invalid("invalid native Error role call"));
                }
                let dst = dst.ok_or_else(|| invalid("Error.text requires result"))?;
                return write(dst, Value::Str(Rc::from(failure.message.as_str())));
            }
            let Some(witness) = engine.role_witness(receiver_value, *role)? else {
                // `Cancelled` рантайма в программе без его объявления: только Error.text.
                if is_runtime_cancelled(receiver_value) && declaration.is_error() {
                    return Ok(Step::Write(*dst, Value::Str(Rc::from("Operation was cancelled"))));
                }
                return Err(invalid("role witness is missing"));
            };
            let callee = *witness.slots.get(*slot as usize).ok_or_else(|| invalid("role slot is missing"))?;
            let mut arguments = vec![receiver_value.clone()];
            arguments.extend(values(args)?);
            Ok(Step::Call {
                function: callee.0 as usize,
                args: arguments,
                dst: *dst,
            })
        }
        I::Await { dst, future } => {
            let Value::Future(future) = r(*future)? else {
                return Err(invalid("await of a non-future value"));
            };
            let future = future.clone();
            let value = sched::await_future(&future).map_err(Stop::Unwind)?;
            Ok(Step::Write(*dst, value))
        }
        I::FutSpawn { dst, future } => {
            let value = r(*future)?.clone();
            let Value::Future(future) = &value else {
                return Err(invalid("spawn of a non-future value"));
            };
            sched::spawn(future);
            write(*dst, value)
        }
        I::Fut { op, dst, future } => {
            let Value::Future(future) = r(*future)? else {
                return Err(invalid("future operation on a non-future"));
            };
            let future = future.clone();
            let value = match op {
                okkama_sparrow::FutOp::Cancel => {
                    sched::cancel(&future);
                    Value::Undef
                }
                okkama_sparrow::FutOp::Done => Value::Bool(future.done()),
                okkama_sparrow::FutOp::Cancelled => Value::Bool(future.cancelled()),
            };
            Ok(Step::Write(*dst, value))
        }
        I::WorkerSpawn { dst, entry, args, direct } => {
            let entry = r(*entry)?.clone();
            let symbol = |function: usize| program.functions.get(function).map(|function| function.symbol.clone());
            let spawned = okkama_runtime_core::worker::target(&entry, values(args)?, *direct, &symbol)
                .and_then(|(target, all)| okkama_runtime_core::worker::spawn(target, all, &|value| type_name(program, value)));
            match spawned {
                Ok(value) => write(*dst, value),
                Err(failure) => Err(Stop::Unwind(failure)),
            }
        }
        I::WorkerBind { dst, action, entry, fields } => {
            write(*dst, okkama_runtime_core::worker::bind(r(*action)?, r(*entry)?.clone(), values(fields)?).map_err(invalid)?)
        }
        I::RegionEnter { dst } => write(*dst, Value::Region(Rc::new(Region::new()))),
        I::RegionAdopt { region, value } | I::RegionTrack { region, value } => {
            let Value::Region(region) = r(*region)? else {
                return Err(invalid("expected region value"));
            };
            let value = r(*value)?;
            if matches!(instruction, I::RegionAdopt { .. }) {
                region.adopt(value);
            } else {
                region.track(value);
            }
            Ok(Step::Write(None, Value::Undef))
        }
        I::RegionTransfer { from, to, value } => {
            let (Value::Region(from), Value::Region(to)) = (r(*from)?, r(*to)?) else {
                return Err(invalid("expected region values"));
            };
            from.transfer(to, r(*value)?);
            Ok(Step::Write(None, Value::Undef))
        }
        I::RegionLeave { .. } | I::RegionUnwind { .. } => unreachable!("handled by close_region"),
        I::FailSecondary { dst, primary, cleanup } => {
            write(*dst, Value::Failure(failure(r(*primary)?)?.with_secondary(failure(r(*cleanup)?)?.clone())))
        }
        I::FailCaused { dst, new, handled } => write(*dst, Value::Failure(Failure::caused(failure(r(*new)?)?, failure(r(*handled)?)?))),
        I::Jump { target } => Ok(Step::Transfer(target, None)),
        I::Branch { condition, then_target, else_target } => {
            Ok(Step::Transfer(if r(*condition)?.as_bool()? { then_target } else { else_target }, None))
        }
        I::Return { value } => Ok(Step::Return(value.map(|value| r(value).cloned()).transpose()?)),
        I::Panic { message } => Ok(Step::Raise(Failure::new(Kind::Panic(string(r(*message)?)?.to_owned())))),
        I::Trap { message, .. } => Ok(Step::Raise(Failure::new(Kind::Program(message.clone())))),
        I::Unreachable {} => Err(Stop::Unsupported("terminator Unreachable is not supported yet".into())),
        I::Raise { failure: value } => Ok(Step::Raise(failure(r(*value)?)?.clone())),
        I::Throw { value } => {
            let value = r(*value)?;
            if let Value::NativeFailure(failure) = value {
                return Ok(Step::Raise(Failure::new(Kind::Native(failure.clone()))));
            }
            let declaration = engine.object_declaration(value)?;
            let object = value.as_object()?;
            let raised = if object.declaration == okkama_runtime_core::heap::CANCELLED {
                Failure::new(Kind::Thrown {
                    object: Ref(object.clone()),
                    name: "Cancelled".into(),
                    message: Some("Operation was cancelled".into()),
                })
            } else {
                Failure::new(Kind::Thrown {
                    object: Ref(object.clone()),
                    name: okkama_runtime_core::failure::error_type_name(&program.objects[declaration].name),
                    message: None,
                })
            };
            Ok(Step::Raise(raised))
        }
        I::Catch { failure: value, kind, ty, value: payload, handler, next } => {
            let raised = failure(r(*value)?)?;
            if let Kind::Native(native) = &raised.kind {
                let matches = match kind {
                    CatchKind::Object => program.objects[*ty as usize].is_native_failure(),
                    CatchKind::Role => program.roles[*ty as usize].is_error(),
                };
                return Ok(if matches {
                    Step::Transfer(handler, Some((payload.0, Value::NativeFailure(native.clone()))))
                } else {
                    Step::Transfer(next, None)
                });
            }
            if let Kind::Thrown { object, .. } = &raised.kind {
                let caught = Value::Object(object.0.clone());
                let matches = match kind {
                    CatchKind::Object => engine.object_declaration(&caught)? == *ty as usize,
                    CatchKind::Role => {
                        engine.role_witness(&caught, RoleRef(*ty))?.is_some()
                            || (is_runtime_cancelled(&caught) && program.roles[*ty as usize].is_error())
                    }
                };
                return Ok(if matches {
                    Step::Transfer(handler, Some((payload.0, caught)))
                } else {
                    Step::Transfer(next, None)
                });
            }
            Ok(Step::Transfer(next, None))
        }
        I::ArrayLookup { array, index, value, found, missing } => {
            let index = int(r(*index)?)?;
            Ok(match ops::array_lookup(r(*array)?, index)? {
                Some(item) => Step::Transfer(found, Some((value.0, item.clone()))),
                None => Step::Transfer(missing, None),
            })
        }
        I::MapLookup { map, key, value, found, missing } => Ok(match ops::map_lookup(r(*map)?, r(*key)?)? {
            Some(item) => Step::Transfer(found, Some((value.0, item.clone()))),
            None => Step::Transfer(missing, None),
        }),
    }
}

pub(crate) fn execute_function(engine: &Rc<Engine>, entry: usize, args: Vec<Value>) -> Result<Option<Value>, Located> {
    let program = &engine.program;
    let at = |frames: &Vec<Frame>, stop: Stop| {
        let (function, pc) = frames.last().map_or((entry, 0), |frame| (frame.function, frame.pc.saturating_sub(1)));
        Located { function, pc, stop }
    };
    let mut frames = vec![Frame::new(program, entry, args, None).map_err(|stop| Located { function: entry, pc: 0, stop })?];
    loop {
        let (index, pc) = {
            let frame = frames.last().expect("entry returns before the stack is empty");
            (frame.function, frame.pc)
        };
        let function = &program.functions[index];
        let Some(instruction) = function.code.get(pc) else {
            return Err(at(&frames, invalid("execution ran past the end of the code")));
        };
        frames.last_mut().expect("frame").pc = pc + 1;
        // Место инструкции: отказ, дошедший до корня, сообщается здесь;
        // ошибка вложенного машинного кода сохраняет своё место.
        let here = |stop| match stop {
            Stop::Located(located) => *located,
            stop => Located { function: index, pc, stop },
        };
        if matches!(instruction, I::RegionLeave { .. } | I::RegionUnwind { .. }) {
            if let Err(stop) = close_region(engine, &mut frames, pc, instruction) {
                return Err(here(stop));
            }
            continue;
        }
        let step = match engine.prepared(index) {
            Ok(prepared) => evaluate(engine, function, &prepared.constants, pc, &frames.last().expect("frame").registers, instruction),
            Err(stop) => Err(stop),
        };
        let step = match step {
            Ok(step) => step,
            Err(stop) => {
                let raised = into_failure(stop).map_err(here)?;
                propagate(program, &mut frames, raised).map_err(here)?;
                continue;
            }
        };
        let mut returned = None;
        let applied = match step {
            Step::Write(dst, value) => {
                if let Some(dst) = dst {
                    frames.last_mut().expect("frame").registers[dst.0.0 as usize] = value;
                }
                Ok(())
            }
            Step::Call { function, args, dst } => call(engine, &mut frames, function, args, dst),
            Step::Transfer(target, payload) => {
                let moved = transfer(frames.last_mut().expect("frame"), target, payload);
                // Обратный переход горячего цикла: остаток вызова — машинным кодом.
                if moved.is_ok()
                    && target.pc as usize <= pc
                    && let Some(jit) = &engine.jit
                    && let Some(code) = jit.backedge(engine, index)
                    && frames.last().is_some_and(|frame| frame.closing.is_none())
                    && crate::jit::stack_allows()
                {
                    let frame = frames.last_mut().expect("frame");
                    match jit.enter(code, &mut frame.registers, target.pc, function.result.is_some()) {
                        Ok(value) => returned = Some(value),
                        Err(Exit::Raise(failure, origin)) => match leave_frame(program, &mut frames, failure) {
                            Ok(()) => {}
                            Err(stop) => return Err(Located { function: origin.0, pc: origin.1, stop }),
                        },
                        Err(Exit::Fatal(located)) => return Err(located),
                    }
                }
                moved
            }
            Step::Raise(failure) => propagate(program, &mut frames, failure),
            Step::Return(value) => {
                returned = Some(value);
                Ok(())
            }
        };
        let applied = match returned {
            Some(returned) => {
                let destination = frames.last().expect("frame").destination;
                frames.pop();
                match frames.last_mut() {
                    Some(caller) => match (destination, returned) {
                        (Some(register), Some(value)) => {
                            caller.registers[register.0 as usize] = value;
                            Ok(())
                        }
                        (None, None) => Ok(()),
                        _ => Err(invalid("call result and return disagree")),
                    },
                    None => return Ok(returned),
                }
            }
            None => applied,
        };
        if let Err(stop) = applied {
            // Отказ до корня или ошибка исполнителя — с местом этой инструкции.
            match into_failure(stop) {
                Ok(raised) => propagate(program, &mut frames, raised).map_err(here)?,
                Err(stop) => return Err(here(stop)),
            }
        }
    }
}

/// Отказ вышел из функции верхнего кадра (её обработчики уже проверены
/// машинным кодом): снять кадр и распространить отказ дальше.
fn leave_frame(program: &Program, frames: &mut Vec<Frame>, failure: Rc<Failure>) -> Result<(), Stop> {
    let frame = frames.pop().expect("frame");
    if frame.boundary {
        let caller = frames
            .last_mut()
            .and_then(|caller| caller.closing.as_mut())
            .ok_or_else(|| invalid("destroy returned outside region close"))?;
        caller.failures.push(failure);
        return Ok(());
    }
    propagate(program, frames, failure)
}

/// Описанный отказ: тексты Error-объектов вычислены (`Error.text`).
pub(crate) fn describe_failure(engine: &Rc<Engine>, failure: &Rc<Failure>) -> Result<Rc<Failure>, Located> {
    let program = &engine.program;
    let located = |stop| Located { function: program.entry.map_or(0, |entry| entry.0 as usize), pc: 0, stop };
    let mut described = (**failure).clone();
    if let Kind::Thrown { object, message, .. } = &mut described.kind {
        if message.is_none() {
            let value = Value::Object(object.0.clone());
            let (role, declaration) = program
                .roles
                .iter()
                .enumerate()
                .find(|(_, role)| role.is_error())
                .ok_or_else(|| located(invalid("Error role is missing")))?;
            let slot = declaration
                .methods
                .iter()
                .position(|method| method.name == "text")
                .ok_or_else(|| located(invalid("Error.text is missing")))?;
            let function = engine
                .role_witness(&value, RoleRef(role as u32))
                .map_err(located)?
                .and_then(|witness| witness.slots.get(slot))
                .copied()
                .ok_or_else(|| located(invalid("Error.text witness is missing")))?;
            let text = execute_function(engine, function.0 as usize, vec![value])
                .map_err(|_| located(Stop::Unsupported("failure while formatting Error.text is not supported yet".into())))?
                .ok_or_else(|| located(invalid("Error.text returned void")))?;
            *message = Some(text.as_string().map_err(|error| located(invalid(error.message)))?.to_owned());
        }
    }
    described.secondary = RefCell::new(
        failure
            .secondaries()
            .iter()
            .map(|secondary| describe_failure(engine, secondary))
            .collect::<Result<_, _>>()?,
    );
    Ok(Rc::new(described))
}

/// Публичная ошибка запуска: место и описанный отказ.
pub(crate) fn public_error(engine: &Rc<Engine>, located: Located) -> Error {
    let function = engine
        .program
        .functions
        .get(located.function)
        .map_or_else(String::new, |function| function.symbol.clone());
    let error = match located.stop {
        Stop::Unwind(failure) => match describe_failure(engine, &failure) {
            Ok(described) => OperationError::Unwind(described),
            Err(error) => return public_error(engine, error),
        },
        other => stop_error(&other),
    };
    Error::Operation {
        function,
        instruction: located.pc,
        error,
    }
}

fn run_entry(engine: &Rc<Engine>, entry: usize) -> Result<IntValue, Located> {
    let outcome = if engine.program.functions[entry].is_async {
        // `async main`: корневой Future и цикл планировщика до его исхода.
        let root = cold_call(engine, entry, Vec::new());
        sched::run_main(&root).map(Some).map_err(|failure| {
            *engine.uncaught.borrow_mut() = Some(failure.clone());
            Located {
                function: entry,
                pc: 0,
                stop: failure_stop(failure),
            }
        })
    } else {
        invoke(engine, entry, Vec::new()).map_err(|located| {
            if let Stop::Unwind(failure) = &located.stop {
                *engine.uncaught.borrow_mut() = Some(failure.clone());
            }
            Located {
                stop: root_stop(located.stop),
                ..located
            }
        })
    };
    let value = outcome?.ok_or_else(|| Located {
        function: entry,
        pc: 0,
        stop: invalid("entry returned no value"),
    })?;
    match value {
        Value::Int(value) => Ok(value),
        _ => Err(Located {
            function: entry,
            pc: 0,
            stop: invalid("entry returned a non-integer value"),
        }),
    }
}

/// Проверяет программу и исполняет её точку входа.
pub fn execute(program: &Program) -> Result<IntValue, Error> {
    execute_with_arguments(program, Vec::new())
}

/// То же с аргументами командной строки программы.
pub fn execute_with_arguments(program: &Program, arguments: Vec<String>) -> Result<IntValue, Error> {
    execute_traced(program, arguments).0
}

/// [`execute_with_arguments`] и стек вызовов неперехваченного отказа
/// (от места возникновения к `main`; пусто, если отказа не было).
pub fn execute_traced(program: &Program, arguments: Vec<String>) -> (Result<IntValue, Error>, Vec<Site>) {
    let mut trace = Vec::new();
    let result = execute_inner(program, arguments, &mut trace);
    (result, trace)
}

fn execute_inner(program: &Program, arguments: Vec<String>, trace: &mut Vec<Site>) -> Result<IntValue, Error> {
    program.verify().map_err(Error::InvalidProgram)?;
    if program.tests.is_some() {
        return Err(Error::Operation {
            function: String::new(),
            instruction: 0,
            error: OperationError::Unsupported {
                message: "test mode programs run with run_tests".into(),
            },
        });
    }
    let entry = program.entry.ok_or(Error::MissingEntry)?.0 as usize;
    let engine = new_engine(program);
    if okkama_runtime_core::worker::active() {
        serve(&engine);
    }
    engine.set_arguments(arguments);
    sched::reset();
    let result = sched::on_large_stack(|| run_entry(&engine, entry).map_err(|located| public_error(&engine, located)));
    if let Some(jit) = &engine.jit {
        jit.report();
    }
    // Незавершённые задачи освобождаются до возврата: их стеки держат значения.
    sched::reset();
    if result.is_err()
        && let Some(failure) = engine.uncaught.borrow_mut().take()
    {
        *trace = failure.trace.sites();
    }
    if let Err(fatal) = engine.take_fatal() {
        return Err(public_error(&engine, fatal));
    }
    result
}

/// Обслужить запрос воркера и завершить процесс.
pub(crate) fn serve(engine: &Rc<Engine>) -> ! {
    let program = &engine.program;
    okkama_runtime_core::worker::serve(
        |target, args| {
            let function = program
                .functions
                .iter()
                .position(|function| function.symbol == target)
                .ok_or_else(|| format!("runtime: unknown spawn entry {target}"))?;
            match sched::on_large_stack(|| execute_function(engine, function, args)) {
                Ok(value) => Ok(value.unwrap_or(Value::Undef)),
                Err(located) => Err(match into_failure(located.stop) {
                    Ok(failure) => match describe_failure(engine, &failure) {
                        Ok(described) => format!("runtime: {}", okkama_runtime_core::failure::describe(&described)),
                        Err(error) => format!("runtime: {}", stop_error(&error.stop)),
                    },
                    Err(fatal) => format!("runtime: {}", stop_error(&fatal)),
                }),
            }
        },
        &|value| type_name(program, value),
    )
}
