//! Машинный код горячих функций Sparrow (Cranelift): базовый JIT.
//!
//! Функция компилируется целиком после `THRESHOLD` вызовов или обратных
//! переходов. Регистры машинного кода — те же слоты `Value`, что у кадра
//! интерпретатора, поэтому вход возможен в начале функции и в любой цели
//! перехода (OSR горячего цикла), а любая инструкция без быстрого пути
//! исполняется помощником над тем же `evaluate`, что и в интерпретаторе.
//!
//! Быстрые пути в самом коде: константы, копии и арифметика малых целых
//! (переполнение signed-64 уходит в точный помощник, R01), сравнения,
//! переходы по `bool`, возврат и прямой вызов обычной функции (через таблицу
//! кода; ещё не скомпилированная функция — через интерпретатор). Значение
//! копируется словами только после проверки тега: значения кучи всегда
//! клонирует и освобождает рантайм.
//!
//! Отказ: помощник кладёт его в `Ctx` и возвращает `RAISE`; обработчик
//! диапазона инструкции принимает его в коде, иначе функция возвращает
//! `RAISE` вызывающему. Ошибка исполнителя — `FATAL` с местом.
//!
//! Стек: машинный код исполняется на стеке текущей задачи. Перед вложенным
//! вызовом проверяется запас до нижней границы стека (её сообщает
//! планировщик); без запаса вызов исполняет интерпретатор с кадрами в куче,
//! поэтому глубина рекурсии ограничена памятью, как без JIT.
#![allow(unsafe_code)]

use crate::machine::{self, Engine, Exit, Located, Step, Stop, invalid};
use cranelift_codegen::ir::{
    self, AbiParam, Block, BlockArg, InstBuilder, JumpTableData, MemFlagsData, StackSlotData, StackSlotKind,
    condcodes::{FloatCC, IntCC}, types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, default_libcall_names};
use okkama_runtime_core::failure::Failure;
use okkama_runtime_core::{IntValue, Value, sched};
use okkama_sparrow::{Cmp, Function, Instruction as I, Program, Register, Target};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

const OK: u32 = 0;
const RAISE: u32 = 1;
const FATAL: u32 = 2;
const SECOND: u32 = 3;

/// Размер слота регистра (`Value`, `repr(C, u8)`).
const SLOT: i32 = 24;
/// Запас стека, ниже которого вложенный вызов исполняет интерпретатор.
const MARGIN: usize = 4 << 20;
/// Наибольшее число регистров вызываемой функции, освобождаемых в коде.
const INLINE_RELEASE: usize = 32;

/// Теги раскладки `Value`, прочитанные с образцов значений.
#[derive(Debug, Clone, Copy)]
struct Layout {
    undef: u8,
    int: u8,
    bool_: u8,
    float: u8,
    small: u8,
    /// Биты тегов значений без ссылок (кроме `int`, у которого — по виду).
    plain: u64,
    /// Раскладка `Value::Object`, если её удалось прочитать с образца.
    object: Option<ObjectLayout>,
    /// Раскладка `Value::Array` (те же поля: тег, `Rc`, указатель и длина).
    array: Option<ObjectLayout>,
}

fn probe_array() -> Option<ObjectLayout> {
    let mut elements = Vec::with_capacity(13);
    elements.resize(7, Value::Undef);
    let data = elements.as_ptr() as usize;
    let value = okkama_runtime_core::heap::ArrayHeap.allocate(elements);
    let Value::Array(array) = &value else { return None };
    // SAFETY: payload `Value::Array` — указатель `Rc` по смещению 8.
    let pointer = unsafe { *(&value as *const Value).cast::<u8>().add(8).cast::<usize>() };
    let address = Rc::as_ptr(array) as usize;
    let inner = address.checked_sub(pointer).filter(|inner| *inner <= 64)? as i32;
    let words = std::mem::size_of::<okkama_runtime_core::heap::Array>() / 8;
    // SAFETY: чтение слов внутри живого массива.
    let word = |index: usize| unsafe { *(address as *const usize).add(index) };
    let find = |wanted: usize| {
        let found: Vec<usize> = (0..words).filter(|&index| word(index) == wanted).collect();
        (found.len() == 1).then(|| found[0] as i32 * 8)
    };
    Some(ObjectLayout { tag: tag_of(&value), inner, data: find(data)?, len: find(7)?, declaration: find(0x5a5a).unwrap_or(-1) })
}

/// `Value::Object`: тег, затем указатель `Rc` (смещение 8) на блок
/// `RcInner { strong, weak, Object }`; в `Object` — указатель и длина полей.
#[derive(Debug, Clone, Copy)]
struct ObjectLayout {
    tag: u8,
    /// От указателя `Rc` до `Object`.
    inner: i32,
    /// В `Object`: указатель на поля и их число.
    data: i32,
    len: i32,
    /// В `Object`: номер объявления; −1 — не найден (у массива не нужен).
    declaration: i32,
}

fn probe_object() -> Option<ObjectLayout> {
    let mut fields = Vec::with_capacity(13);
    fields.resize(7, Value::Undef);
    let data = fields.as_ptr() as usize;
    let value = okkama_runtime_core::heap::ObjectHeap.allocate(0x5a5a, fields);
    let Value::Object(object) = &value else { return None };
    // SAFETY: payload `Value::Object` — указатель `Rc` по смещению 8.
    let pointer = unsafe { *(&value as *const Value).cast::<u8>().add(8).cast::<usize>() };
    let address = Rc::as_ptr(object) as usize;
    let inner = address.checked_sub(pointer).filter(|inner| *inner <= 64)? as i32;
    let words = std::mem::size_of::<okkama_runtime_core::heap::Object>() / 8;
    // SAFETY: чтение слов внутри живого объекта.
    let word = |index: usize| unsafe { *(address as *const usize).add(index) };
    let find = |wanted: usize| {
        let found: Vec<usize> = (0..words).filter(|&index| word(index) == wanted).collect();
        (found.len() == 1).then(|| found[0] as i32 * 8)
    };
    Some(ObjectLayout { tag: tag_of(&value), inner, data: find(data)?, len: find(7)?, declaration: find(0x5a5a).unwrap_or(-1) })
}

fn tag_of(value: &Value) -> u8 {
    // SAFETY: `Value` — `repr(C, u8)`: первый байт — тег варианта.
    unsafe { *(value as *const Value).cast::<u8>() }
}

fn byte_at(value: &Value, offset: usize) -> u8 {
    // SAFETY: смещение внутри 24-байтового значения.
    unsafe { *(value as *const Value).cast::<u8>().add(offset) }
}

fn probe() -> Option<Layout> {
    if std::mem::size_of::<Value>() != SLOT as usize || std::mem::align_of::<Value>() != 8 {
        return None;
    }
    let small = Value::Int(IntValue::from_i64(0x1234_5678_9abc));
    // SAFETY: payload `IntValue::Small` (`repr(C, u8)`) — i64 по смещению 16.
    let payload = unsafe { *(&small as *const Value).cast::<u8>().add(16).cast::<i64>() };
    let big = Value::Int(IntValue::parse("100000000000000000000000000000")?);
    let truth = Value::Bool(true);
    let plain = [
        Value::Undef,
        Value::Float(1.5),
        Value::Bool(false),
        Value::Byte(1),
        Value::Char('x'),
        Value::Enum { declaration: 0, case: 0 },
    ]
    .iter()
    .fold(0u64, |bits, value| bits | 1 << tag_of(value));
    let layout = Layout {
        object: probe_object(),
        array: probe_array(),
        undef: tag_of(&Value::Undef),
        float: tag_of(&Value::Float(1.5)),
        int: tag_of(&small),
        bool_: tag_of(&truth),
        small: byte_at(&small, 8),
        plain,
    };
    let float = Value::Float(1.5);
    // SAFETY: payload `Value::Float` — f64 по смещению 8.
    let float_bits = unsafe { *(&float as *const Value).cast::<u8>().add(8).cast::<u64>() };
    let consistent = payload == 0x1234_5678_9abc
        && float_bits == 1.5f64.to_bits()
        && byte_at(&truth, 8) == 1
        && byte_at(&Value::Bool(false), 8) == 0
        && byte_at(&big, 8) != layout.small
        && tag_of(&big) == layout.int
        && layout.plain & (1 << layout.int) == 0
        && tag_of(&Value::Str(Rc::from("x"))) < 64
        && layout.plain & (1 << tag_of(&Value::Str(Rc::from("x")))) == 0;
    consistent.then_some(layout)
}

/// Контекст машинного кода. Поля 0 и 8 читает сам код.
#[repr(C)]
#[derive(Debug)]
pub(crate) struct Ctx {
    /// Адреса кода по номеру функции (0 — не скомпилирована).
    table: *const Cell<usize>,
    /// Адрес нижней границы текущего стека (`sched::stack_floor`).
    floor: *const usize,
    engine: *const Engine,
    failure: RefCell<Option<Rc<Failure>>>,
    /// Свободные векторы регистров для динамических вызовов.
    pool: RefCell<Vec<Vec<Value>>>,
    /// Наибольшее число регистров среди целей замыканий (функций из
    /// `fn.closure` программы); 0 — быстрого пути `call.fn` нет.
    closure_registers: usize,
    /// `OKKAMA_JIT_STATS`: выходы машинного кода в помощники по инструкциям.
    stats: Option<RefCell<std::collections::HashMap<String, u64>>>,
    /// Место возникновения отказа `failure`.
    origin: Cell<(usize, usize)>,
    fatal: RefCell<Option<Located>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Cold,
    Compiled,
    Failed,
}

/// Компилятор и код одного исполнителя.
pub(crate) struct Jit {
    module: RefCell<Option<JITModule>>,
    builder: RefCell<FunctionBuilderContext>,
    table: Box<[Cell<usize>]>,
    counts: Box<[Cell<u32>]>,
    states: Box<[Cell<State>]>,
    ctx: Box<Ctx>,
    /// Таблицы кода по ролям (`RoleTables`).
    role_maps: RoleTables,
    layout: Layout,
    threshold: u32,
    debug: bool,
}

impl std::fmt::Debug for Jit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jit").field("threshold", &self.threshold).finish_non_exhaustive()
    }
}

impl Jit {
    /// Печать `OKKAMA_JIT_STATS` в конце запуска.
    pub(crate) fn report(&self) {
        if let Some(stats) = &self.ctx.stats {
            let mut rows: Vec<_> = stats.borrow().iter().map(|(name, count)| (*count, name.clone())).collect();
            rows.sort_unstable_by(|a, b| b.cmp(a));
            eprintln!("jit: helper calls by instruction");
            for (count, name) in rows.iter().take(30) {
                eprintln!("{count:>12}  {name}");
            }
        }
    }
}

impl Drop for Jit {
    fn drop(&mut self) {
        if let Some(module) = self.module.get_mut().take() {
            // SAFETY: исполнитель уничтожается — его код больше не исполняется.
            unsafe { module.free_memory() };
        }
    }
}

/// Порог вызовов (и обратных переходов) до компиляции.
const THRESHOLD: u32 = 1000;

type Entry = unsafe extern "C" fn(*const Ctx, *mut Value, *mut Value, u32) -> u32;

/// Запас текущего стека позволяет войти в машинный код.
pub(crate) fn stack_allows() -> bool {
    let floor = sched::stack_floor();
    let marker = 0u8;
    let sp = &marker as *const u8 as usize;
    floor != 0 && sp > floor + 2 * MARGIN
}

impl Jit {
    /// `OKKAMA_JIT=off` выключает, `eager` компилирует при первом вызове.
    pub(crate) fn new(program: &Program, engine: *const Engine) -> Option<Box<Self>> {
        let mode = std::env::var("OKKAMA_JIT").unwrap_or_default();
        if mode == "off" {
            return None;
        }
        let layout = probe()?;
        let mut flags = settings::builder();
        flags.set("opt_level", "speed").ok()?;
        flags.set("use_colocated_libcalls", "false").ok()?;
        flags.set("is_pic", "false").ok()?;
        let isa = cranelift_native::builder().ok()?.finish(settings::Flags::new(flags)).ok()?;
        let module = JITModule::new(JITBuilder::with_isa(isa, default_libcall_names()));
        let count = program.functions.len();
        let table: Box<[Cell<usize>]> = (0..count).map(|_| Cell::new(0)).collect();
        let ctx = Box::new(Ctx {
            table: table.as_ptr(),
            floor: sched::floor_address(),
            engine,
            failure: RefCell::new(None),
            pool: RefCell::new(Vec::new()),
            closure_registers: closure_registers(program),
            stats: std::env::var_os("OKKAMA_JIT_STATS").map(|_| RefCell::new(std::collections::HashMap::new())),
            origin: Cell::new((0, 0)),
            fatal: RefCell::new(None),
        });
        Some(Box::new(Self {
            module: RefCell::new(Some(module)),
            builder: RefCell::new(FunctionBuilderContext::new()),
            table,
            counts: (0..count).map(|_| Cell::new(0)).collect(),
            states: (0..count).map(|_| Cell::new(State::Cold)).collect(),
            ctx,
            role_maps: RefCell::new(std::collections::HashMap::new()),
            layout,
            threshold: if mode == "eager" { 1 } else { THRESHOLD },
            debug: std::env::var_os("OKKAMA_JIT_DEBUG").is_some(),
        }))
    }

    /// Код функции, ставшей горячей (вызов или обратный переход).
    pub(crate) fn hot(&self, engine: &Engine, index: usize) -> Option<usize> {
        let code = self.table[index].get();
        if code != 0 {
            return Some(code);
        }
        if self.states[index].get() != State::Cold {
            return None;
        }
        let count = self.counts[index].get() + 1;
        self.counts[index].set(count);
        if count < self.threshold {
            return None;
        }
        self.compile(engine, index)
    }

    /// Обратный переход интерпретатора в функции `index`.
    pub(crate) fn backedge(&self, engine: &Engine, index: usize) -> Option<usize> {
        self.hot(engine, index)
    }

    fn compile(&self, engine: &Engine, index: usize) -> Option<usize> {
        let program = &engine.program;
        let function = &program.functions[index];
        let eligible = function.native.is_none() && function.intrinsic.is_none() && !function.code.is_empty();
        let prepared = engine.prepared(index).ok();
        let result = match (eligible, prepared) {
            (true, Some(prepared)) => self.build(program, index, &prepared.constants),
            _ => Err("not eligible".to_string()),
        };
        match result {
            Ok(code) => {
                self.table[index].set(code);
                self.states[index].set(State::Compiled);
                if self.debug {
                    eprintln!("jit: compiled {} ({} instructions)", function.symbol, function.code.len());
                }
                Some(code)
            }
            Err(error) => {
                self.states[index].set(State::Failed);
                if self.debug {
                    eprintln!("jit: {} not compiled: {error}", function.symbol);
                }
                None
            }
        }
    }

    fn build(&self, program: &Program, index: usize, constants: &[Value]) -> Result<usize, String> {
        let mut slot = self.module.borrow_mut();
        let module = slot.as_mut().ok_or("module is gone")?;
        let mut context = module.make_context();
        let pointer = types::I64;
        let mut signature = module.make_signature();
        for ty in [pointer, pointer, pointer, types::I32] {
            signature.params.push(AbiParam::new(ty));
        }
        signature.returns.push(AbiParam::new(types::I32));
        context.func.signature = signature.clone();
        let id = module
            .declare_function(&format!("sparrow_{index}"), Linkage::Local, &signature)
            .map_err(|error| error.to_string())?;
        let config = module.target_config();
        {
            let mut builder_context = self.builder.borrow_mut();
            let builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
            let mut generator = Gen::new(builder, &self.layout, program, index, constants, &signature, &self.role_maps);
            generator.function()?;
            generator.b.seal_all_blocks();
            generator.b.finalize(config);
        }
        module.define_function(id, &mut context).map_err(|error| format!("{error:?}"))?;
        module.clear_context(&mut context);
        module.finalize_definitions().map_err(|error| error.to_string())?;
        Ok(module.get_finalized_function(id) as usize)
    }

    /// Исполнить машинный код над регистрами кадра с инструкции `entry`.
    /// `returns` — у функции есть результат.
    pub(crate) fn enter(&self, code: usize, registers: &mut [Value], entry: u32, returns: bool) -> Result<Option<Value>, Exit> {
        // SAFETY: `code` — адрес функции, скомпилированной с сигнатурой `Entry`.
        let function: Entry = unsafe { std::mem::transmute::<usize, Entry>(code) };
        let mut out = Value::Undef;
        sched::native_enter();
        // SAFETY: регистры — слоты кадра этой функции (их число задаёт
        // программа), `out` — инициализированный слот результата.
        let status = unsafe { function(&*self.ctx, registers.as_mut_ptr(), &mut out, entry) };
        sched::native_leave();
        match status {
            OK => Ok(returns.then_some(out)),
            RAISE => Err(Exit::Raise(self.take_failure(), self.ctx.origin.get())),
            _ => Err(Exit::Fatal(self.ctx.fatal.borrow_mut().take().unwrap_or_else(|| Located {
                function: 0,
                pc: 0,
                stop: invalid("machine code stopped without an error"),
            }))),
        }
    }

    fn take_failure(&self) -> Rc<Failure> {
        self.ctx
            .failure
            .borrow_mut()
            .take()
            .unwrap_or_else(|| Failure::new(okkama_runtime_core::failure::Kind::Program("machine code raised without a failure".into())))
    }
}

// ----- помощники машинного кода -----

/// Исполнитель контекста (`Rc` — для вложенных вызовов).
///
/// SAFETY: `ctx` — контекст живого исполнителя: код исполняется только из
/// его методов.
unsafe fn engine(ctx: *const Ctx) -> std::mem::ManuallyDrop<Rc<Engine>> {
    // Вид на `Rc` исполнителя без изменения счётчиков: исполнитель живёт,
    // пока исполняется его код, и этот вид никогда не освобождается.
    unsafe { std::mem::ManuallyDrop::new(Rc::from_raw((*ctx).engine)) }
}

/// Регистры функции `function` по адресу кадра.
///
/// SAFETY: `regs` — слоты кадра этой функции.
unsafe fn registers<'a>(regs: *mut Value, function: &Function) -> &'a mut [Value] {
    unsafe { std::slice::from_raw_parts_mut(regs, function.registers.len()) }
}

fn settle(ctx: &Ctx, exit: Exit) -> u32 {
    match exit {
        Exit::Raise(failure, origin) => {
            *ctx.failure.borrow_mut() = Some(failure);
            ctx.origin.set(origin);
            RAISE
        }
        Exit::Fatal(located) => {
            *ctx.fatal.borrow_mut() = Some(located);
            FATAL
        }
    }
}

fn stopped(ctx: &Ctx, stop: Stop, at: (usize, usize)) -> u32 {
    settle(ctx, machine::stop_exit(stop, at))
}

/// Учесть выход в помощник (`OKKAMA_JIT_STATS`).
// Без журнала (`OKKAMA_JIT_DEBUG`) — одна проверка в теле помощника.
#[inline(always)]
fn note(ctx: *const Ctx, label: impl FnOnce(&Engine) -> String) {
    // SAFETY: контекст живёт, пока исполняется код его исполнителя.
    let ctx = unsafe { &*ctx };
    if let Some(stats) = &ctx.stats {
        // SAFETY: см. `engine`.
        let engine = unsafe { &*ctx.engine };
        *stats.borrow_mut().entry(label(engine)).or_default() += 1;
    }
}

#[inline(always)]
fn note_at(ctx: *const Ctx, func: u32, pc: u32) {
    note(ctx, |engine| engine.program.functions[func as usize].code[pc as usize].mnemonic().to_string());
}

/// Тело помощника: паника Rust не проходит через кадры машинного кода.
fn guarded(ctx: *const Ctx, at: (usize, usize), body: impl FnOnce(&Ctx) -> u32) -> u32 {
    // SAFETY: контекст живёт, пока исполняется код его исполнителя.
    let ctx = unsafe { &*ctx };
    match catch_unwind(AssertUnwindSafe(|| body(ctx))) {
        Ok(status) => status,
        Err(_) => stopped(ctx, invalid("internal error in machine-code helper"), at),
    }
}

/// Инструкция без быстрого пути — через `evaluate`.
extern "C" fn h_eval(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        // SAFETY: см. `engine`, `registers`.
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let prepared = match engine.prepared(at.0) {
            Ok(prepared) => prepared,
            Err(stop) => return stopped(ctx, stop, at),
        };
        match machine::evaluate(&engine, function, &prepared.constants, at.1, registers, &function.code[at.1]) {
            Ok(Step::Write(dst, value)) => {
                if let Some(dst) = dst {
                    registers[dst.0.0 as usize] = value;
                }
                OK
            }
            Ok(Step::Raise(failure)) => settle(ctx, Exit::Raise(failure, at)),
            Ok(_) => stopped(ctx, invalid("control step in machine-code helper"), at),
            Err(stop) => stopped(ctx, stop, at),
        }
    })
}

/// Первая цель ветвящейся инструкции (`OK`), остальные — `SECOND`.
fn first_target(instruction: &I) -> Option<&Target> {
    match instruction {
        I::Jump { target } => Some(target),
        I::Branch { then_target, .. } => Some(then_target),
        I::Catch { handler, .. } => Some(handler),
        I::ArrayLookup { found, .. } | I::MapLookup { found, .. } => Some(found),
        _ => None,
    }
}

fn second_target(instruction: &I) -> Option<&Target> {
    match instruction {
        I::Branch { else_target, .. } => Some(else_target),
        I::Catch { next, .. } => Some(next),
        I::ArrayLookup { missing, .. } | I::MapLookup { missing, .. } => Some(missing),
        _ => None,
    }
}

/// Ветвление целиком: вычисление, записи ребра и значение ребра.
extern "C" fn h_branch(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let instruction = &function.code[at.1];
        let prepared = match engine.prepared(at.0) {
            Ok(prepared) => prepared,
            Err(stop) => return stopped(ctx, stop, at),
        };
        match machine::evaluate(&engine, function, &prepared.constants, at.1, registers, instruction) {
            Ok(Step::Transfer(target, payload)) => {
                let first = first_target(instruction).is_some_and(|first| std::ptr::eq(first, target));
                match machine::transfer_registers(registers, target, payload) {
                    Ok(()) if first => OK,
                    Ok(()) => SECOND,
                    Err(stop) => stopped(ctx, stop, at),
                }
            }
            Ok(Step::Raise(failure)) => settle(ctx, Exit::Raise(failure, at)),
            Ok(_) => stopped(ctx, invalid("unexpected step of a branch"), at),
            Err(stop) => stopped(ctx, stop, at),
        }
    })
}

/// Принять отказ обработчиком диапазона инструкции `pc`.
extern "C" fn h_handler(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let Some(handler) = function.handler_at(pc) else {
            return stopped(ctx, invalid("handler is missing"), at);
        };
        let Some(failure) = ctx.failure.borrow_mut().take() else {
            return stopped(ctx, invalid("handler without a failure"), at);
        };
        match machine::transfer_registers(registers, &handler.target, Some((handler.failure, Value::Failure(failure)))) {
            Ok(()) => OK,
            Err(stop) => stopped(ctx, stop, at),
        }
    })
}

/// Записать кадр в стек вызовов текущего отказа (при `FATAL` отказа нет).
extern "C" fn h_trace(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    guarded(ctx, (func as usize, pc as usize), |ctx| {
        if let Some(failure) = ctx.failure.borrow().as_ref() {
            failure.trace.record(regs as usize, okkama_runtime_core::failure::Site { function: func, pc });
        }
        OK
    })
}

/// Любой вызов без быстрого пути: `call.cap`, native, intrinsic, async.
extern "C" fn h_call(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| call_generic(ctx, regs, at))
}

/// Вызов native-функции: аргументы берутся прямо из регистров и уходят
/// provider, без шага интерпретатора.
extern "C" fn h_native(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let I::Call { dst, function: callee, args } = &function.code[at.1] else {
            return call_generic(ctx, regs, at);
        };
        let mut values = Vec::with_capacity(args.len());
        for register in args {
            match machine::read(registers, *register) {
                Ok(value) => values.push(value.clone()),
                Err(stop) => return stopped(ctx, stop, at),
            }
        }
        match machine::call_now(&engine, callee.0 as usize, values, at) {
            Ok(returned) => match (dst, returned) {
                (Some(dst), Some(value)) => {
                    registers[dst.0.0 as usize] = value;
                    OK
                }
                (Some(_), None) => stopped(ctx, invalid("call result and return disagree"), at),
                (None, _) => OK,
            },
            Err(exit) => settle(ctx, exit),
        }
    })
}

/// Цель `call.role` по receiver (номер функции) или −1: тогда общий путь.
extern "C" fn h_role_target(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    let at = (func as usize, pc as usize);
    // Паника — тоже «нет цели»: общий путь сообщит ошибку сам.
    catch_unwind(AssertUnwindSafe(|| {
        let ctx = unsafe { &*ctx };
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let I::CallRole { receiver, role, slot, .. } = &function.code[at.1] else {
            return u32::MAX;
        };
        let value = &registers[receiver.0 as usize];
        if !matches!(value, Value::Object(_)) {
            return u32::MAX;
        }
        match engine.role_witness(value, *role) {
            Ok(Some(witness)) => witness.slots.get(*slot as usize).map_or(u32::MAX, |callee| callee.0),
            _ => u32::MAX,
        }
    }))
    .unwrap_or(u32::MAX)
}

/// Наибольшее число регистров среди функций, на которые указывают
/// `fn.closure` программы (цели `call.fn`), если не больше 512.
fn closure_registers(program: &Program) -> usize {
    let mut targets = std::collections::BTreeSet::new();
    for function in &program.functions {
        for instruction in &function.code {
            if let I::Closure { function, .. } = instruction {
                targets.insert(function.0 as usize);
            }
        }
    }
    let largest = targets.iter().map(|index| program.functions[*index].registers.len()).max().unwrap_or(0);
    if largest <= 512 { largest } else { 0 }
}

/// `call.fn` с целью из набора `fn.closure`: захваты (копией) и аргументы
/// (перемещением по `mask`) пишутся в регистры вызываемой `area`; результат —
/// номер цели или `u32::MAX` (тогда общий путь, регистры не тронуты).
extern "C" fn h_fn_frame(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32, area: *mut Value, mask: u64) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        let ctx = unsafe { &*ctx };
        let engine = unsafe { engine(ctx) };
        let program = &engine.program;
        let function = &program.functions[func as usize];
        let registers = unsafe { registers(regs, function) };
        let I::CallFn { callee, args, .. } = &function.code[pc as usize] else {
            return u32::MAX;
        };
        let Value::Fn(closure) = &registers[callee.0 as usize] else {
            return u32::MAX;
        };
        let target = &program.functions[closure.function];
        let count = target.registers.len();
        if !plain(target) || count > ctx.closure_registers || target.params.len() != closure.captures.len() + args.len() {
            return u32::MAX;
        }
        let index = closure.function as u32;
        // SAFETY: `area` — регистры на `closure_registers` слотов без значений.
        let frame = unsafe { std::slice::from_raw_parts_mut(area, count) };
        let mut params = target.params.iter();
        for value in &closure.captures {
            frame[params.next().expect("arity checked").0 as usize] = value.clone();
        }
        for (position, arg) in args.iter().enumerate() {
            let moved = position < 64 && mask & (1 << position) != 0;
            frame[params.next().expect("arity checked").0 as usize] = owned(registers, *arg, moved);
        }
        index
    }))
    .unwrap_or(u32::MAX)
}

/// Обычная функция: не async, не native и не intrinsic.
fn plain(function: &Function) -> bool {
    !function.is_async && function.native.is_none() && function.intrinsic.is_none()
}

/// `call.fn` и `call.role` с целью — обычной функцией: захваты (или
/// receiver) и аргументы копируются прямо в регистры вызываемой из пула,
/// без промежуточных векторов; остальное — общий путь.
extern "C" fn h_call_dyn(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let program = &engine.program;
        let function = &program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        // Цель и префикс аргументов: `Rc` замыкания (его захваты) или receiver.
        enum Prefix {
            Captures(Rc<okkama_runtime_core::heap::Closure>),
            Receiver(Value),
        }
        let (target, dst, args, prefix): (usize, _, &[Register], Prefix) = match &function.code[at.1] {
            I::CallFn { dst, callee, args } => match &registers[callee.0 as usize] {
                Value::Fn(closure) => (closure.function, *dst, args, Prefix::Captures(closure.clone())),
                _ => return call_generic(ctx, regs, at),
            },
            I::CallRole { dst, receiver, role, slot, args } => {
                let value = &registers[receiver.0 as usize];
                if !matches!(value, Value::Object(_)) {
                    return call_generic(ctx, regs, at);
                }
                match engine.role_witness(value, *role) {
                    Ok(Some(witness)) => match witness.slots.get(*slot as usize) {
                        Some(callee) => (callee.0 as usize, *dst, args, Prefix::Receiver(value.clone())),
                        None => return call_generic(ctx, regs, at),
                    },
                    _ => return call_generic(ctx, regs, at),
                }
            }
            _ => return call_generic(ctx, regs, at),
        };
        let prefix_len = match &prefix {
            Prefix::Captures(closure) => closure.captures.len(),
            Prefix::Receiver(_) => 1,
        };
        let callee = &program.functions[target];
        if !plain(callee) || callee.params.len() != prefix_len + args.len() {
            return call_generic(ctx, regs, at);
        }
        let mut frame = ctx.pool.borrow_mut().pop().unwrap_or_default();
        frame.resize(callee.registers.len(), Value::Undef);
        let mut params = callee.params.iter();
        match prefix {
            Prefix::Captures(closure) => {
                for value in &closure.captures {
                    frame[params.next().expect("arity checked").0 as usize] = value.clone();
                }
            }
            Prefix::Receiver(value) => frame[params.next().expect("arity checked").0 as usize] = value,
        }
        for arg in args {
            frame[params.next().expect("arity checked").0 as usize] = registers[arg.0 as usize].clone();
        }
        let returned = run_frame(&engine, target, &mut frame);
        frame.clear();
        {
            let mut pool = ctx.pool.borrow_mut();
            if pool.len() < 256 {
                pool.push(frame);
            }
        }
        match returned {
            Ok(returned) => match (dst, returned) {
                (Some(dst), Some(value)) => {
                    registers[dst.0.0 as usize] = value;
                    OK
                }
                (Some(_), None) => stopped(ctx, invalid("call result and return disagree"), at),
                (None, _) => OK,
            },
            Err(exit) => settle(ctx, exit),
        }
    })
}

/// Исполнить функцию над готовыми регистрами: машинным кодом или интерпретатором.
fn run_frame(engine: &Rc<Engine>, index: usize, frame: &mut [Value]) -> Result<Option<Value>, Exit> {
    let function = &engine.program.functions[index];
    if let Some(jit) = &engine.jit
        && let Some(code) = jit.hot(engine, index)
        && stack_allows()
    {
        return jit.enter(code, frame, 0, function.result.is_some());
    }
    let args = function.params.iter().map(|param| std::mem::replace(&mut frame[param.0 as usize], Value::Undef)).collect();
    machine::invoke(engine, index, args).map_err(machine::exit)
}

fn call_generic(ctx: &Ctx, regs: *mut Value, at: (usize, usize)) -> u32 {
    {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let prepared = match engine.prepared(at.0) {
            Ok(prepared) => prepared,
            Err(stop) => return stopped(ctx, stop, at),
        };
        match machine::evaluate(&engine, function, &prepared.constants, at.1, registers, &function.code[at.1]) {
            Ok(Step::Call { function: callee, args, dst }) => match machine::call_now(&engine, callee, args, at) {
                Ok(returned) => match (dst, returned) {
                    (Some(dst), Some(value)) => {
                        registers[dst.0.0 as usize] = value;
                        OK
                    }
                    (Some(_), None) => stopped(ctx, invalid("call result and return disagree"), at),
                    (None, _) => OK,
                },
                Err(exit) => settle(ctx, exit),
            },
            Ok(Step::Write(dst, value)) => {
                if let Some(dst) = dst {
                    registers[dst.0.0 as usize] = value;
                }
                OK
            }
            Ok(Step::Raise(failure)) => settle(ctx, Exit::Raise(failure, at)),
            Ok(_) => stopped(ctx, invalid("unexpected step of a call"), at),
            Err(stop) => stopped(ctx, stop, at),
        }
    }
}

/// Закрытие региона: `destroy` исполняются вложенно.
extern "C" fn h_region(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        match machine::close_now(&engine, registers, &function.code[at.1], at) {
            Ok(()) => OK,
            Err(exit) => settle(ctx, exit),
        }
    })
}

/// Прямой вызов без машинного кода вызываемой (или без запаса стека).
/// `area` — регистры вызываемой с аргументами в параметрах.
extern "C" fn h_call_slow(ctx: *const Ctx, area: *mut Value, out: *mut Value, callee: u32) -> u32 {
    note(ctx, |_| "call (no code yet / no stack)".to_string());
    let at = (callee as usize, 0);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(area, function) };
        let args = function
            .params
            .iter()
            .map(|param| std::mem::replace(&mut registers[param.0 as usize], Value::Undef))
            .collect();
        match machine::invoke(&engine, at.0, args) {
            Ok(Some(value)) => {
                // SAFETY: `out` — слот результата, инициализированный `Undef`.
                unsafe { *out = value };
                OK
            }
            Ok(None) => OK,
            Err(located) => settle(ctx, machine::exit(located)),
        }
    })
}

/// `obj.next`: бит 0 `mask` — объект больше не читается (его можно обновить
/// на месте при единственной ссылке), бит `i + 1` — значение i-го обновления.
extern "C" fn h_obj_next(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32, mask: u64) -> u32 {
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let I::ObjNext { dst, object, updates } = &function.code[at.1] else {
            return stopped(ctx, invalid("expected obj.next"), at);
        };
        let take = |registers: &mut [Value], register: Register, owned: bool| {
            if owned {
                std::mem::replace(&mut registers[register.0 as usize], Value::Undef)
            } else {
                registers[register.0 as usize].clone()
            }
        };
        let moved = |index: usize| index < 63 && mask & (1 << (index + 1)) != 0;
        // Единственная ссылка: значения пишутся прямо в поля, без промежуточного
        // вектора. Значение обновления из регистра объекта — только копией ниже.
        if mask & 1 != 0 && updates.iter().all(|(_, value)| value != object) {
            let mut source = std::mem::replace(&mut registers[object.0 as usize], Value::Undef);
            let count = match &source {
                Value::Object(object) => object.field_count(),
                _ => 0,
            };
            if updates.iter().all(|(field, _)| (*field as usize) < count)
                && let Some(fields) = okkama_runtime_core::heap::ObjectHeap.unique_fields(&mut source)
            {
                for (index, (field, value)) in updates.iter().enumerate() {
                    fields[*field as usize] = take(registers, *value, moved(index));
                }
                if ctx.stats.is_some() {
                    note(ctx, |_| "obj.next: on the spot".into());
                }
                registers[dst.0.0 as usize] = source;
                return OK;
            }
            registers[object.0 as usize] = source;
        }
        let updates: Vec<(usize, Value)> = updates
            .iter()
            .enumerate()
            .map(|(index, (field, value))| (*field as usize, take(registers, *value, moved(index))))
            .collect();
        let source = take(registers, *object, mask & 1 != 0);
        let before = match &source {
            Value::Object(object) => Rc::as_ptr(object) as usize,
            _ => 0,
        };
        match okkama_runtime_core::heap::ObjectHeap.successor_owned(source, updates) {
            Ok(value) => {
                if ctx.stats.is_some() {
                    let reused = matches!(&value, Value::Object(object) if Rc::as_ptr(object) as usize == before);
                    note(ctx, |_| if reused { "obj.next: on the spot".into() } else { "obj.next: copy".into() });
                }
                registers[dst.0.0 as usize] = value;
                OK
            }
            Err(error) => stopped(ctx, error.into(), at),
        }
    })
}

fn owned(registers: &mut [Value], register: Register, moved: bool) -> Value {
    if moved {
        std::mem::replace(&mut registers[register.0 as usize], Value::Undef)
    } else {
        registers[register.0 as usize].clone()
    }
}

/// `obj.new`: поля — перемещением при последнем использовании (бит `mask`).
extern "C" fn h_obj_new(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32, mask: u64) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let I::ObjNew { dst, object, fields } = &function.code[at.1] else {
            return stopped(ctx, invalid("expected obj.new"), at);
        };
        if engine.program.objects[object.0 as usize].builtin {
            return stopped(ctx, invalid("runtime objects are created by the runtime"), at);
        }
        let values = fields
            .iter()
            .enumerate()
            .map(|(index, field)| owned(registers, *field, index < 64 && mask & (1 << index) != 0))
            .collect();
        registers[dst.0.0 as usize] = okkama_runtime_core::heap::ObjectHeap.allocate(object.0 as usize, values);
        OK
    })
}

/// `builder.push`: builder (бит 0) и значение (бит 1) — перемещением.
extern "C" fn h_builder_push(ctx: *const Ctx, regs: *mut Value, func: u32, pc: u32, mask: u64) -> u32 {
    note_at(ctx, func, pc);
    let at = (func as usize, pc as usize);
    guarded(ctx, at, |ctx| {
        let engine = unsafe { engine(ctx) };
        let function = &engine.program.functions[at.0];
        let registers = unsafe { registers(regs, function) };
        let I::BuilderPush { dst, builder, value } = &function.code[at.1] else {
            return stopped(ctx, invalid("expected builder.push"), at);
        };
        let item = owned(registers, *value, mask & 2 != 0);
        let target = owned(registers, *builder, mask & 1 != 0);
        match okkama_runtime_core::heap::ArrayHeap.builder_push_into(&target, item) {
            Ok(()) => {
                registers[dst.0.0 as usize] = target;
                OK
            }
            Err(error) => stopped(ctx, error.into(), at),
        }
    })
}

/// Недопустимый вход или состояние кода.
extern "C" fn h_fault(ctx: *const Ctx, func: u32, pc: u32) -> u32 {
    let ctx = unsafe { &*ctx };
    stopped(ctx, invalid("invalid machine-code entry or state"), (func as usize, pc as usize))
}

/// Освободить значение слота (слот становится `Undef`).
extern "C" fn h_drop(slot: *mut Value) {
    // SAFETY: слот содержит инициализированное значение.
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        let value = std::ptr::replace(slot, Value::Undef);
        drop(value);
    }));
}

/// Освободить `count` слотов подряд.
extern "C" fn h_drop_range(slots: *mut Value, count: usize) {
    // SAFETY: `slots` — регистры вызываемой функции (`count` слотов).
    let slots = unsafe { std::slice::from_raw_parts_mut(slots, count) };
    for slot in slots {
        // Без ссылок — нечего освобождать (и деструктор не вызывается).
        let plain = match slot {
            Value::Undef | Value::Bool(_) | Value::Float(_) | Value::Byte(_) | Value::Char(_) | Value::Enum { .. } => true,
            Value::Int(value) => value.to_i64().is_some(),
            _ => false,
        };
        if !plain {
            h_drop(slot);
        }
    }
}

/// Записать копию `src` в `dst`, не освобождая прежнее содержимое `dst`.
extern "C" fn h_clone(src: *const Value, dst: *mut Value) {
    // SAFETY: `src` — значение, `dst` — слот без значения кучи.
    unsafe { std::ptr::write(dst, (*src).clone()) };
}

// ----- кодогенерация -----

/// Цель вызова: известна при компиляции или вычислена в коде.
#[derive(Clone, Copy)]
enum Callee {
    Static(u32),
    Dynamic(ir::Value),
}

struct Sigs {
    eval: ir::SigRef,
    slow: ir::SigRef,
    code: ir::SigRef,
    fault: ir::SigRef,
    drop: ir::SigRef,
    clone: ir::SigRef,
    next: ir::SigRef,
    frame: ir::SigRef,
}

/// Таблицы ролей, на которые ссылается машинный код (живут с `Jit`):
/// ключ `role` — битовая карта «объявление делает роль»; ключ
/// `DISPATCH | role << 32 | slot` — функция слота по номеру объявления
/// (`u64::MAX` — объявление роль не делает).
type RoleTables = RefCell<std::collections::HashMap<u64, Box<[u64]>>>;

const DISPATCH: u64 = 1 << 63;

struct Gen<'a, 'b> {
    b: FunctionBuilder<'b>,
    layout: &'a Layout,
    program: &'a Program,
    function: &'a Function,
    index: u32,
    constants: &'a [Value],
    ctx: ir::Value,
    regs: ir::Value,
    out: ir::Value,
    blocks: Vec<Option<Block>>,
    sigs: Sigs,
    /// Живость регистров: последнее использование — перемещение.
    live: crate::liveness::Liveness,
    role_maps: &'a RoleTables,
    /// Регистры прямо вызываемой функции и её результат.
    area: Option<ir::StackSlot>,
    result: Option<ir::StackSlot>,
}

fn flags() -> MemFlagsData {
    MemFlagsData::trusted()
}

fn offset(register: Register) -> i32 {
    register.0 as i32 * SLOT
}

impl<'a, 'b> Gen<'a, 'b> {
    fn new(
        mut b: FunctionBuilder<'b>,
        layout: &'a Layout,
        program: &'a Program,
        index: usize,
        constants: &'a [Value],
        entry: &ir::Signature,
        role_maps: &'a RoleTables,
    ) -> Self {
        let function = &program.functions[index];
        let pointer = types::I64;
        let call_conv = entry.call_conv;
        let mut signature = |params: &[ir::Type], returns: &[ir::Type]| {
            let mut sig = ir::Signature::new(call_conv);
            sig.params.extend(params.iter().map(|ty| AbiParam::new(*ty)));
            sig.returns.extend(returns.iter().map(|ty| AbiParam::new(*ty)));
            b.import_signature(sig)
        };
        let sigs = Sigs {
            eval: signature(&[pointer, pointer, types::I32, types::I32], &[types::I32]),
            slow: signature(&[pointer, pointer, pointer, types::I32], &[types::I32]),
            code: signature(&[pointer, pointer, pointer, types::I32], &[types::I32]),
            fault: signature(&[pointer, types::I32, types::I32], &[types::I32]),
            drop: signature(&[pointer], &[]),
            clone: signature(&[pointer, pointer], &[]),
            next: signature(&[pointer, pointer, types::I32, types::I32, types::I64], &[types::I32]),
            frame: signature(&[pointer, pointer, types::I32, types::I32, pointer, types::I64], &[types::I32]),
        };
        let start = b.create_block();
        b.append_block_params_for_function_params(start);
        b.switch_to_block(start);
        let params = b.block_params(start).to_vec();
        let mut generator = Self {
            b,
            layout,
            program,
            function,
            index: index as u32,
            constants,
            ctx: params[0],
            regs: params[1],
            out: params[2],
            blocks: vec![None; function.code.len()],
            sigs,
            live: crate::liveness::Liveness::new(function),
            role_maps,
            area: None,
            result: None,
        };
        generator.leaders();
        // Вход: начало функции или цель перехода (OSR).
        let bad = generator.b.create_block();
        let calls: Vec<_> = (0..function.code.len())
            .map(|pc| {
                let block = generator.blocks[pc].unwrap_or(bad);
                generator.b.func.dfg.block_call(block, &[])
            })
            .collect();
        let default = generator.b.func.dfg.block_call(bad, &[]);
        let table = generator.b.create_jump_table(JumpTableData::new(default, &calls));
        generator.b.ins().br_table(params[3], table);
        generator.b.switch_to_block(bad);
        let status = generator.fault(0);
        generator.b.ins().return_(&[status]);
        generator
    }

    /// Блоки: начало, цели переходов и обработчиков, инструкции после терминаторов.
    fn leaders(&mut self) {
        let mark = |pc: u32, blocks: &mut Vec<Option<Block>>, b: &mut FunctionBuilder<'b>| {
            if let Some(slot) = blocks.get_mut(pc as usize)
                && slot.is_none()
            {
                *slot = Some(b.create_block());
            }
        };
        mark(0, &mut self.blocks, &mut self.b);
        for (pc, instruction) in self.function.code.iter().enumerate() {
            for target in [first_target(instruction), second_target(instruction)].into_iter().flatten() {
                mark(target.pc, &mut self.blocks, &mut self.b);
            }
            if instruction.is_terminator() {
                mark(pc as u32 + 1, &mut self.blocks, &mut self.b);
            }
        }
        for handler in &self.function.handlers {
            mark(handler.target.pc, &mut self.blocks, &mut self.b);
        }
    }

    fn block(&self, pc: u32) -> Result<Block, String> {
        self.blocks.get(pc as usize).copied().flatten().ok_or_else(|| format!("target {pc} is not a block"))
    }

    fn helper(&mut self, address: usize, sig: ir::SigRef, args: &[ir::Value]) -> Option<ir::Value> {
        let callee = self.b.ins().iconst(types::I64, address as i64);
        let inst = self.b.ins().call_indirect(sig, callee, args);
        self.b.inst_results(inst).first().copied()
    }

    fn i32(&mut self, value: u32) -> ir::Value {
        self.b.ins().iconst(types::I32, i64::from(value))
    }

    fn fault(&mut self, pc: usize) -> ir::Value {
        let (func, pc) = (self.i32(self.index), self.i32(pc as u32));
        let ctx = self.ctx;
        self.helper(h_fault as *const () as usize, self.sigs.fault, &[ctx, func, pc]).expect("status")
    }

    fn eval(&mut self, address: usize, pc: usize) -> ir::Value {
        let (func, pc) = (self.i32(self.index), self.i32(pc as u32));
        let (ctx, regs) = (self.ctx, self.regs);
        self.helper(address, self.sigs.eval, &[ctx, regs, func, pc]).expect("status")
    }

    /// Статус помощника: 0 — дальше, иначе путь отказа инструкции `pc`.
    fn check(&mut self, status: ir::Value, pc: usize) -> Result<(), String> {
        let next = self.b.create_block();
        let (failed, param) = self.failure_entry();
        self.b.ins().brif(status, failed, &[BlockArg::Value(status)], next, &[]);
        self.fill_failure(failed, param, pc)?;
        self.b.switch_to_block(next);
        Ok(())
    }

    /// Блок пути отказа со статусом в параметре (заполняется после перехода).
    fn failure_entry(&mut self) -> (Block, ir::Value) {
        let failed = self.b.create_block();
        let status = self.b.append_block_param(failed, types::I32);
        (failed, status)
    }

    /// Путь отказа: обработчик диапазона или возврат статуса. Текущий блок
    /// должен быть уже завершён.
    fn fill_failure(&mut self, failed: Block, status: ir::Value, pc: usize) -> Result<(), String> {
        self.b.switch_to_block(failed);
        // Кадр машинного кода — в стек вызовов отказа (интерпретатор этих
        // кадров не видит). Путь холодный: только при отказе.
        let _ = self.eval(h_trace as *const () as usize, pc);
        match self.function.handler_at(pc as u32) {
            Some(handler) => {
                let target = self.block(handler.target.pc)?;
                let caught = self.b.create_block();
                let leave = self.b.create_block();
                let left = self.b.append_block_param(leave, types::I32);
                let raised = self.b.ins().icmp_imm_s(IntCC::Equal, status, i64::from(RAISE));
                self.b.ins().brif(raised, caught, &[], leave, &[BlockArg::Value(status)]);
                self.b.switch_to_block(caught);
                let accepted = self.eval(h_handler as *const () as usize, pc);
                self.b.ins().brif(accepted, leave, &[BlockArg::Value(accepted)], target, &[]);
                self.b.switch_to_block(leave);
                self.b.ins().return_(&[left]);
            }
            None => {
                self.b.ins().return_(&[status]);
            }
        }
        Ok(())
    }

    fn load8(&mut self, base: ir::Value, offset: i32) -> ir::Value {
        self.b.ins().uload8(types::I32, flags(), base, offset)
    }

    fn store8(&mut self, value: u8, base: ir::Value, offset: i32) {
        let value = self.b.ins().iconst(types::I8, i64::from(value));
        self.b.ins().store(flags(), value, base, offset);
    }

    /// Малое целое слота: (признак, значение).
    fn small_int(&mut self, base: ir::Value, offset: i32) -> (ir::Value, ir::Value) {
        let tag = self.load8(base, offset);
        let kind = self.load8(base, offset + 8);
        let is_int = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(self.layout.int));
        let is_small = self.b.ins().icmp_imm_s(IntCC::Equal, kind, i64::from(self.layout.small));
        let both = self.b.ins().band(is_int, is_small);
        let value = self.b.ins().load(types::I64, flags(), base, offset + 16);
        (both, value)
    }

    /// Значение слота без ссылок (его можно копировать словами).
    fn plain(&mut self, base: ir::Value, offset: i32) -> ir::Value {
        let tag = self.load8(base, offset);
        let wide = self.b.ins().uextend(types::I64, tag);
        let mask = self.b.ins().iconst(types::I64, self.layout.plain as i64);
        let shifted = self.b.ins().ushr(mask, wide);
        let bit = self.b.ins().band_imm_u(shifted, 1);
        let listed = self.b.ins().icmp_imm_s(IntCC::NotEqual, bit, 0);
        let (small, _) = self.small_int(base, offset);
        self.b.ins().bor(listed, small)
    }

    /// Освободить содержимое слота перед записью (значение кучи — помощником).
    fn release(&mut self, base: ir::Value, offset: i32) {
        let plain = self.plain(base, offset);
        let release = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().brif(plain, done, &[], release, &[]);
        self.b.switch_to_block(release);
        let address = self.b.ins().iadd_imm_s(base, i64::from(offset));
        self.helper(h_drop as *const () as usize, self.sigs.drop, &[address]);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
    }

    fn store_small(&mut self, base: ir::Value, offset: i32, value: ir::Value) {
        self.store8(self.layout.int, base, offset);
        self.store8(self.layout.small, base, offset + 8);
        self.b.ins().store(flags(), value, base, offset + 16);
    }

    fn store_bool(&mut self, base: ir::Value, offset: i32, value: ir::Value) {
        self.store8(self.layout.bool_, base, offset);
        self.b.ins().store(flags(), value, base, offset + 8);
    }

    fn copy_words(&mut self, from: ir::Value, from_offset: i32, to: ir::Value, to_offset: i32) {
        for word in 0..3 {
            let value = self.b.ins().load(types::I64, flags(), from, from_offset + word * 8);
            self.b.ins().store(flags(), value, to, to_offset + word * 8);
        }
    }

    /// Копия значения в слот без значения: словами или помощником.
    fn clone_into(&mut self, from: ir::Value, from_offset: i32, to: ir::Value, to_offset: i32) {
        let plain = self.plain(from, from_offset);
        let words = self.b.create_block();
        let heap = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().brif(plain, words, &[], heap, &[]);
        self.b.switch_to_block(words);
        self.copy_words(from, from_offset, to, to_offset);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(heap);
        let source = self.b.ins().iadd_imm_s(from, i64::from(from_offset));
        let target = self.b.ins().iadd_imm_s(to, i64::from(to_offset));
        self.helper(h_clone as *const () as usize, self.sigs.clone, &[source, target]);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
    }

    fn function(&mut self) -> Result<(), String> {
        let code = &self.function.code;
        let mut open = false;
        for (pc, instruction) in code.iter().enumerate() {
            if let Some(block) = self.blocks[pc] {
                if open {
                    self.b.ins().jump(block, &[]);
                }
                self.b.switch_to_block(block);
            } else if !open {
                continue; // недостижимо без входа
            }
            self.instruction(pc, instruction)?;
            open = !instruction.is_terminator();
        }
        if open {
            let status = self.fault(code.len());
            self.b.ins().return_(&[status]);
        }
        Ok(())
    }

    fn generic(&mut self, pc: usize) -> Result<(), String> {
        let status = self.eval(h_eval as *const () as usize, pc);
        self.check(status, pc)
    }

    /// Терминатор, который только отказывает (`panic`, `throw`, …).
    fn raising(&mut self, pc: usize) -> Result<(), String> {
        self.generic(pc)?;
        let status = self.fault(pc);
        self.b.ins().return_(&[status]);
        Ok(())
    }

    fn instruction(&mut self, pc: usize, instruction: &I) -> Result<(), String> {
        let regs = self.regs;
        match instruction {
            I::Const { dst, .. } => match &self.constants[pc] {
                Value::Int(value) if value.to_i64().is_some() => {
                    let value = self.b.ins().iconst(types::I64, value.to_i64().expect("small"));
                    self.release(regs, offset(dst.0));
                    self.store_small(regs, offset(dst.0), value);
                }
                Value::Bool(value) => {
                    let value = self.b.ins().iconst(types::I8, i64::from(*value));
                    self.release(regs, offset(dst.0));
                    self.store_bool(regs, offset(dst.0), value);
                }
                Value::Float(value) => {
                    let bits = self.b.ins().iconst(types::I64, value.to_bits() as i64);
                    self.release(regs, offset(dst.0));
                    self.store8(self.layout.float, regs, offset(dst.0));
                    self.b.ins().store(flags(), bits, regs, offset(dst.0) + 8);
                }
                // Готовое значение константы (живёт, пока жив исполнитель).
                constant => {
                    let address = self.b.ins().iconst(types::I64, constant as *const Value as i64);
                    self.release(regs, offset(dst.0));
                    self.clone_into(address, 0, regs, offset(dst.0));
                }
            },
            I::FloatAdd { dst, left, right } | I::FloatSub { dst, left, right } | I::FloatMul { dst, left, right } | I::FloatDiv { dst, left, right } => {
                self.float_binary(pc, instruction, dst.0, *left, *right)?;
            }
            I::FloatNeg { dst, operand } | I::FloatAbs { dst, operand } => {
                let regs = self.regs;
                let (ok, value) = self.float_operand(regs, offset(*operand));
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                let done = self.b.create_block();
                self.b.ins().brif(ok, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let result = if matches!(instruction, I::FloatNeg { .. }) { self.b.ins().fneg(value) } else { self.b.ins().fabs(value) };
                self.store_float(regs, offset(dst.0), result);
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(slow);
                self.generic(pc)?;
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
            }
            I::FloatCmp { op, dst, left, right } => {
                let regs = self.regs;
                let (a_ok, a) = self.float_operand(regs, offset(*left));
                let (b_ok, b) = self.float_operand(regs, offset(*right));
                let both = self.b.ins().band(a_ok, b_ok);
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                let done = self.b.create_block();
                self.b.ins().brif(both, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                // IEEE: `-0.0 == 0.0`; NaN в языке не бывает.
                let cc = match op {
                    Cmp::Eq => FloatCC::Equal,
                    Cmp::Ne => FloatCC::NotEqual,
                    Cmp::Lt => FloatCC::LessThan,
                    Cmp::Le => FloatCC::LessThanOrEqual,
                    Cmp::Gt => FloatCC::GreaterThan,
                    Cmp::Ge => FloatCC::GreaterThanOrEqual,
                };
                let result = self.b.ins().fcmp(cc, a, b);
                self.release(regs, offset(dst.0));
                self.store_bool(regs, offset(dst.0), result);
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(slow);
                self.generic(pc)?;
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
            }
            I::Undef { dst } => {
                self.release(regs, offset(dst.0));
                self.store8(self.layout.undef, regs, offset(dst.0));
            }
            I::Move { dst, source } if dst.0 != *source && !self.live.live_after(pc, *source) => {
                // Последнее использование: значение переходит словами.
                self.release(regs, offset(dst.0));
                self.copy_words(regs, offset(*source), regs, offset(dst.0));
                self.store8(self.layout.undef, regs, offset(*source));
            }
            I::ObjNext { object, updates, .. } => {
                let mut mask = 0u64;
                if !self.live.live_after(pc, *object) {
                    mask |= 1;
                }
                for (index, (_, value)) in updates.iter().enumerate().take(63) {
                    let once = updates.iter().filter(|(_, other)| other == value).count() == 1 && value != object;
                    if once && !self.live.live_after(pc, *value) {
                        mask |= 1 << (index + 1);
                    }
                }
                let (func, at) = (self.i32(self.index), self.i32(pc as u32));
                let mask = self.b.ins().iconst(types::I64, mask as i64);
                let ctx = self.ctx;
                let status = self.helper(h_obj_next as *const () as usize, self.sigs.next, &[ctx, regs, func, at, mask]).expect("status");
                self.check(status, pc)?;
            }
            I::Move { dst, source } if dst.0 != *source => {
                let plain = self.plain(regs, offset(*source));
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                let done = self.b.create_block();
                self.b.ins().brif(plain, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                self.release(regs, offset(dst.0));
                self.copy_words(regs, offset(*source), regs, offset(dst.0));
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(slow);
                self.generic(pc)?;
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
            }
            I::RoleWrap { dst, source, role } | I::RoleNarrow { dst, source, role, .. }
                if dst.0 != *source && self.layout.object.is_some_and(|object| object.declaration >= 0) =>
            {
                self.role_check(pc, dst.0, *source, role.0)?;
            }
            I::ObjNew { fields, .. } => {
                let mask = self.dying_mask(pc, fields);
                self.masked(h_obj_new as *const () as usize, pc, mask)?;
            }
            I::BuilderPush { builder, value, .. } => {
                let mask = self.dying_mask(pc, &[*builder, *value]);
                self.masked(h_builder_push as *const () as usize, pc, mask)?;
            }
            I::ArrayGet { dst, array, index } if dst.0 != *array && dst.0 != *index && self.layout.array.is_some() => {
                self.array_get(pc, dst.0, *array, *index)?;
            }
            I::ArrayLen { dst, array } if dst.0 != *array && self.layout.array.is_some() => {
                let layout = self.layout.array.expect("checked");
                let regs = self.regs;
                let tag = self.load8(regs, offset(*array));
                let is_array = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(layout.tag));
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                let done = self.b.create_block();
                self.b.ins().brif(is_array, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let pointer = self.b.ins().load(types::I64, flags(), regs, offset(*array) + 8);
                let len = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.len);
                self.release(regs, offset(dst.0));
                self.store_small(regs, offset(dst.0), len);
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(slow);
                self.generic(pc)?;
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
            }
            I::ObjGet { dst, object, field } if dst.0 != *object && self.layout.object.is_some() => {
                self.object_get(pc, dst.0, *object, *field)?;
            }
            I::IntAdd { dst, left, right } | I::IntSub { dst, left, right } | I::IntMul { dst, left, right } | I::IntMod { dst, left, right } => {
                self.arithmetic(pc, instruction, dst.0, *left, *right)?;
            }
            I::IntCmp { op, dst, left, right } => {
                let (a_ok, a) = self.small_int(regs, offset(*left));
                let (b_ok, b) = self.small_int(regs, offset(*right));
                let both = self.b.ins().band(a_ok, b_ok);
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                let done = self.b.create_block();
                self.b.ins().brif(both, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let cc = match op {
                    Cmp::Eq => IntCC::Equal,
                    Cmp::Ne => IntCC::NotEqual,
                    Cmp::Lt => IntCC::SignedLessThan,
                    Cmp::Le => IntCC::SignedLessThanOrEqual,
                    Cmp::Gt => IntCC::SignedGreaterThan,
                    Cmp::Ge => IntCC::SignedGreaterThanOrEqual,
                };
                let result = self.b.ins().icmp(cc, a, b);
                self.release(regs, offset(dst.0));
                self.store_bool(regs, offset(dst.0), result);
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(slow);
                self.generic(pc)?;
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
            }
            // Равенство значений: два малых целых сравниваются в коде
            // (обобщённый `==` над `T = int`), остальное — `evaluate`.
            I::ValCmp { op, dst, left, right } => {
                let (a_ok, a) = self.small_int(regs, offset(*left));
                let (b_ok, b) = self.small_int(regs, offset(*right));
                let both = self.b.ins().band(a_ok, b_ok);
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                let done = self.b.create_block();
                self.b.ins().brif(both, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let cc = if *op == Cmp::Eq { IntCC::Equal } else { IntCC::NotEqual };
                let result = self.b.ins().icmp(cc, a, b);
                self.release(regs, offset(dst.0));
                self.store_bool(regs, offset(dst.0), result);
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(slow);
                self.generic(pc)?;
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
            }
            I::Jump { target } => {
                self.edge(target)?;
            }
            I::Branch { condition, then_target, else_target } => {
                let tag = self.load8(regs, offset(*condition));
                let is_bool = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(self.layout.bool_));
                let fast = self.b.create_block();
                let slow = self.b.create_block();
                self.b.ins().brif(is_bool, fast, &[], slow, &[]);
                self.b.switch_to_block(fast);
                let value = self.load8(regs, offset(*condition) + 8);
                let then_block = self.b.create_block();
                let else_block = self.b.create_block();
                self.b.ins().brif(value, then_block, &[], else_block, &[]);
                self.b.switch_to_block(then_block);
                self.edge(then_target)?;
                self.b.switch_to_block(else_block);
                self.edge(else_target)?;
                self.b.switch_to_block(slow);
                self.branching(pc, then_target.pc, else_target.pc)?;
            }
            I::Return { value } => {
                if let Some(value) = value {
                    let out = self.out;
                    self.copy_words(regs, offset(*value), out, 0);
                    self.store8(self.layout.undef, regs, offset(*value));
                }
                let status = self.i32(OK);
                self.b.ins().return_(&[status]);
            }
            I::Catch { handler, next, .. } => self.branching(pc, handler.pc, next.pc)?,
            I::ArrayLookup { found, missing, .. } | I::MapLookup { found, missing, .. } => {
                self.branching(pc, found.pc, missing.pc)?;
            }
            I::Panic { .. } | I::Trap { .. } | I::Throw { .. } | I::Raise { .. } | I::Unreachable {} => self.raising(pc)?,
            I::Call { dst, function, args } if self.direct(function.0 as usize) => {
                self.direct_call(pc, dst.map(|dst| dst.0), function.0, args)?;
            }
            I::CallRole { dst, receiver, role, slot, args } if self.role_call(pc, dst.map(|dst| dst.0), role.0, *slot, *receiver, args)? => {}
            I::CallFn { dst, callee, args } if self.closure_call(pc, dst.map(|dst| dst.0), *callee, args)? => {}
            I::CallFn { .. } | I::CallRole { .. } => {
                let status = self.eval(h_call_dyn as *const () as usize, pc);
                self.check(status, pc)?;
            }
            I::Call { function, .. } if self.program.functions[function.0 as usize].native.is_some() => {
                let status = self.eval(h_native as *const () as usize, pc);
                self.check(status, pc)?;
            }
            I::Call { .. } | I::CallCap { .. } => {
                let status = self.eval(h_call as *const () as usize, pc);
                self.check(status, pc)?;
            }
            I::RegionLeave { .. } | I::RegionUnwind { .. } => {
                let status = self.eval(h_region as *const () as usize, pc);
                self.check(status, pc)?;
            }
            _ => self.generic(pc)?,
        }
        Ok(())
    }

    /// `obj.get`: поле конструктора читается из объекта прямо в коде;
    /// не объект, позднее поле или выход за число полей — помощник.
    fn object_get(&mut self, pc: usize, dst: Register, object: Register, field: u32) -> Result<(), String> {
        let layout = self.layout.object.expect("checked by caller");
        let regs = self.regs;
        let slow = self.b.create_block();
        let done = self.b.create_block();
        let tag = self.load8(regs, offset(object));
        let is_object = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(layout.tag));
        let pointer_block = self.b.create_block();
        self.b.ins().brif(is_object, pointer_block, &[], slow, &[]);
        self.b.switch_to_block(pointer_block);
        let pointer = self.b.ins().load(types::I64, flags(), regs, offset(object) + 8);
        let len = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.len);
        let inside = self.b.ins().icmp_imm_u(IntCC::UnsignedGreaterThan, len, i64::from(field));
        let field_block = self.b.create_block();
        self.b.ins().brif(inside, field_block, &[], slow, &[]);
        self.b.switch_to_block(field_block);
        let data = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.data);
        let at = field as i32 * SLOT;
        let field_tag = self.load8(data, at);
        let bound = self.b.ins().icmp_imm_s(IntCC::NotEqual, field_tag, i64::from(self.layout.undef));
        let copy = self.b.create_block();
        self.b.ins().brif(bound, copy, &[], slow, &[]);
        self.b.switch_to_block(copy);
        self.release(regs, offset(dst));
        self.clone_into(data, at, regs, offset(dst));
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        self.generic(pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    /// Значение `float` слота: (признак, f64).
    fn float_operand(&mut self, base: ir::Value, offset: i32) -> (ir::Value, ir::Value) {
        let tag = self.load8(base, offset);
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(self.layout.float));
        let value = self.b.ins().load(types::F64, flags(), base, offset + 8);
        (ok, value)
    }

    /// Записать `float` (прежнее значение — `float` или без ссылок).
    fn store_float(&mut self, base: ir::Value, offset: i32, value: ir::Value) {
        self.release(base, offset);
        self.store8(self.layout.float, base, offset);
        self.b.ins().store(flags(), value, base, offset + 8);
    }

    /// `float.add/sub/mul/div`: конечный результат — в коде, иначе помощник
    /// (он и сообщает ошибку программы).
    fn float_binary(&mut self, pc: usize, instruction: &I, dst: Register, left: Register, right: Register) -> Result<(), String> {
        let regs = self.regs;
        let (a_ok, a) = self.float_operand(regs, offset(left));
        let (b_ok, b) = self.float_operand(regs, offset(right));
        let both = self.b.ins().band(a_ok, b_ok);
        let fast = self.b.create_block();
        let write = self.b.create_block();
        let result = self.b.append_block_param(write, types::F64);
        let slow = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().brif(both, fast, &[], slow, &[]);
        self.b.switch_to_block(fast);
        let value = match instruction {
            I::FloatAdd { .. } => self.b.ins().fadd(a, b),
            I::FloatSub { .. } => self.b.ins().fsub(a, b),
            I::FloatMul { .. } => self.b.ins().fmul(a, b),
            _ => {
                let zero = self.b.ins().f64const(0.0);
                let divisor = self.b.ins().fcmp(FloatCC::NotEqual, b, zero);
                let divide = self.b.create_block();
                self.b.ins().brif(divisor, divide, &[], slow, &[]);
                self.b.switch_to_block(divide);
                self.b.ins().fdiv(a, b)
            }
        };
        // Конечность: |x| < ∞ (NaN не проходит сравнение).
        let magnitude = self.b.ins().fabs(value);
        let infinity = self.b.ins().f64const(f64::INFINITY);
        let finite = self.b.ins().fcmp(FloatCC::LessThan, magnitude, infinity);
        self.b.ins().brif(finite, write, &[BlockArg::Value(value)], slow, &[]);
        self.b.switch_to_block(write);
        self.store_float(regs, offset(dst), result);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        self.generic(pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    /// `arr.get` с малым неотрицательным индексом в границах — в коде;
    /// остальное (отказ границ, не массив) — помощник.
    fn array_get(&mut self, pc: usize, dst: Register, array: Register, index: Register) -> Result<(), String> {
        let layout = self.layout.array.expect("checked by caller");
        let regs = self.regs;
        let slow = self.b.create_block();
        let done = self.b.create_block();
        let tag = self.load8(regs, offset(array));
        let is_array = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(layout.tag));
        let (small, position) = self.small_int(regs, offset(index));
        let both = self.b.ins().band(is_array, small);
        let bounds = self.b.create_block();
        self.b.ins().brif(both, bounds, &[], slow, &[]);
        self.b.switch_to_block(bounds);
        let pointer = self.b.ins().load(types::I64, flags(), regs, offset(array) + 8);
        let len = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.len);
        // Беззнаково: отрицательный индекс тоже за границей.
        let inside = self.b.ins().icmp(IntCC::UnsignedLessThan, position, len);
        let copy = self.b.create_block();
        self.b.ins().brif(inside, copy, &[], slow, &[]);
        self.b.switch_to_block(copy);
        let data = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.data);
        let scaled = self.b.ins().imul_imm_s(position, i64::from(SLOT));
        let element = self.b.ins().iadd(data, scaled);
        self.release(regs, offset(dst));
        self.clone_into(element, 0, regs, offset(dst));
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        self.generic(pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    /// Маска регистров, чьё значение после `pc` не читается (до 64; повтор — нет).
    fn dying_mask(&self, pc: usize, registers: &[Register]) -> u64 {
        registers
            .iter()
            .enumerate()
            .take(64)
            .filter(|(_, register)| registers.iter().filter(|other| other == register).count() == 1)
            .filter(|(_, register)| !self.live.live_after(pc, **register))
            .fold(0, |mask, (index, _)| mask | 1 << index)
    }

    /// Помощник `(ctx, regs, func, pc, mask) -> status` и проверка статуса.
    fn masked(&mut self, address: usize, pc: usize, mask: u64) -> Result<(), String> {
        let (func, at) = (self.i32(self.index), self.i32(pc as u32));
        let mask = self.b.ins().iconst(types::I64, mask as i64);
        let (ctx, regs) = (self.ctx, self.regs);
        let status = self.helper(address, self.sigs.next, &[ctx, regs, func, at, mask]).expect("status");
        self.check(status, pc)
    }

    /// Цель `call.role` по таблице слота: объявление объекта-receiver —
    /// индекс; не объект, объявление вне таблицы или без роли — помощник.
    /// Результат — номер функции (i32), отрицательный — общий путь.
    fn role_dispatch(&mut self, pc: usize, layout: ObjectLayout, role: u32, slot: u32, receiver: Register) -> ir::Value {
        let program = self.program;
        let count = program.objects.len();
        let address = {
            let mut maps = self.role_maps.borrow_mut();
            let key = DISPATCH | u64::from(role) << 32 | u64::from(slot);
            let table = maps.entry(key).or_insert_with(|| {
                program
                    .objects
                    .iter()
                    .map(|object| {
                        object
                            .does
                            .iter()
                            .find(|witness| witness.role.0 == role)
                            .and_then(|witness| witness.slots.get(slot as usize))
                            .map_or(u64::MAX, |callee| u64::from(callee.0))
                    })
                    .collect()
            });
            table.as_ptr() as i64
        };
        let regs = self.regs;
        let done = self.b.create_block();
        let result = self.b.append_block_param(done, types::I32);
        let miss = self.b.create_block();
        let tag = self.load8(regs, offset(receiver));
        let is_object = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(layout.tag));
        let declared = self.b.create_block();
        self.b.ins().brif(is_object, declared, &[], miss, &[]);
        self.b.switch_to_block(declared);
        let pointer = self.b.ins().load(types::I64, flags(), regs, offset(receiver) + 8);
        let declaration = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.declaration);
        let known = self.b.ins().icmp_imm_u(IntCC::UnsignedLessThan, declaration, count as i64);
        let lookup = self.b.create_block();
        self.b.ins().brif(known, lookup, &[], miss, &[]);
        self.b.switch_to_block(lookup);
        let scaled = self.b.ins().ishl_imm_u(declaration, 3);
        let table = self.b.ins().iconst(types::I64, address);
        let entry = self.b.ins().iadd(table, scaled);
        let callee = self.b.ins().load(types::I64, flags(), entry, 0);
        let absent = self.b.ins().icmp_imm_s(IntCC::Equal, callee, -1);
        let found = self.b.create_block();
        self.b.ins().brif(absent, miss, &[], found, &[]);
        self.b.switch_to_block(found);
        let narrow = self.b.ins().ireduce(types::I32, callee);
        self.b.ins().jump(done, &[BlockArg::Value(narrow)]);
        self.b.switch_to_block(miss);
        let slow = self.eval(h_role_target as *const () as usize, pc);
        self.b.ins().jump(done, &[BlockArg::Value(slow)]);
        self.b.switch_to_block(done);
        result
    }

    /// `role.wrap`/`role.narrow` объекта: делает ли объявление роль — бит
    /// карты, построенной из таблиц программы; иначе (и не объект) — помощник.
    fn role_check(&mut self, pc: usize, dst: Register, source: Register, role: u32) -> Result<(), String> {
        let layout = self.layout.object.expect("checked by caller");
        let program = self.program;
        let count = program.objects.len();
        let address = {
            let mut maps = self.role_maps.borrow_mut();
            let map = maps.entry(u64::from(role)).or_insert_with(|| {
                let mut bits = vec![0u64; count.div_ceil(64).max(1)].into_boxed_slice();
                for (index, object) in program.objects.iter().enumerate() {
                    if object.does.iter().any(|witness| witness.role.0 == role) {
                        bits[index / 64] |= 1 << (index % 64);
                    }
                }
                bits
            });
            map.as_ptr() as i64
        };
        let regs = self.regs;
        let slow = self.b.create_block();
        let done = self.b.create_block();
        let tag = self.load8(regs, offset(source));
        let is_object = self.b.ins().icmp_imm_s(IntCC::Equal, tag, i64::from(layout.tag));
        let declared = self.b.create_block();
        self.b.ins().brif(is_object, declared, &[], slow, &[]);
        self.b.switch_to_block(declared);
        let pointer = self.b.ins().load(types::I64, flags(), regs, offset(source) + 8);
        let declaration = self.b.ins().load(types::I64, flags(), pointer, layout.inner + layout.declaration);
        let known = self.b.ins().icmp_imm_u(IntCC::UnsignedLessThan, declaration, count as i64);
        let lookup = self.b.create_block();
        self.b.ins().brif(known, lookup, &[], slow, &[]);
        self.b.switch_to_block(lookup);
        let word_index = self.b.ins().ushr_imm_u(declaration, 6);
        let word_offset = self.b.ins().ishl_imm_u(word_index, 3);
        let map = self.b.ins().iconst(types::I64, address);
        let word_address = self.b.ins().iadd(map, word_offset);
        let word = self.b.ins().load(types::I64, flags(), word_address, 0);
        let bit_index = self.b.ins().band_imm_u(declaration, 63);
        let shifted = self.b.ins().ushr(word, bit_index);
        let bit = self.b.ins().band_imm_u(shifted, 1);
        let fast = self.b.create_block();
        self.b.ins().brif(bit, fast, &[], slow, &[]);
        self.b.switch_to_block(fast);
        self.release(regs, offset(dst));
        if self.live.live_after(pc, source) {
            self.clone_into(regs, offset(source), regs, offset(dst));
        } else {
            self.copy_words(regs, offset(source), regs, offset(dst));
            self.store8(self.layout.undef, regs, offset(source));
        }
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        self.generic(pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    fn arithmetic(&mut self, pc: usize, instruction: &I, dst: Register, left: Register, right: Register) -> Result<(), String> {
        let regs = self.regs;
        let (a_ok, a) = self.small_int(regs, offset(left));
        let (b_ok, b) = self.small_int(regs, offset(right));
        let both = self.b.ins().band(a_ok, b_ok);
        let fast = self.b.create_block();
        let write = self.b.create_block();
        let result = self.b.append_block_param(write, types::I64);
        let slow = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().brif(both, fast, &[], slow, &[]);
        self.b.switch_to_block(fast);
        let (value, overflow) = match instruction {
            I::IntAdd { .. } => self.b.ins().sadd_overflow(a, b),
            I::IntSub { .. } => self.b.ins().ssub_overflow(a, b),
            I::IntMul { .. } => self.b.ins().smul_overflow(a, b),
            _ => {
                // Усечённый остаток; делитель 0 и −1 — точным помощником.
                let zero = self.b.ins().icmp_imm_s(IntCC::Equal, b, 0);
                let minus = self.b.ins().icmp_imm_s(IntCC::Equal, b, -1);
                let special = self.b.ins().bor(zero, minus);
                let divide = self.b.create_block();
                self.b.ins().brif(special, slow, &[], divide, &[]);
                self.b.switch_to_block(divide);
                let value = self.b.ins().srem(a, b);
                self.b.ins().jump(write, &[BlockArg::Value(value)]);
                return self.finish_arithmetic(pc, write, result, slow, done, dst);
            }
        };
        self.b.ins().brif(overflow, slow, &[], write, &[BlockArg::Value(value)]);
        self.finish_arithmetic(pc, write, result, slow, done, dst)
    }

    fn finish_arithmetic(&mut self, pc: usize, write: Block, result: ir::Value, slow: Block, done: Block, dst: Register) -> Result<(), String> {
        let regs = self.regs;
        self.b.switch_to_block(write);
        self.release(regs, offset(dst));
        self.store_small(regs, offset(dst), result);
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        self.generic(pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    /// Ребро перехода: параллельные записи и переход. Тождественные записи
    /// пропускаются; если все источники без ссылок — копии словами, иначе
    /// снимок источников во временные слоты (кучу клонирует рантайм).
    fn edge(&mut self, target: &Target) -> Result<(), String> {
        let block = self.block(target.pc)?;
        let moves: Vec<(Register, Register)> = target.moves.iter().copied().filter(|(dst, source)| dst != source).collect();
        if moves.is_empty() {
            self.b.ins().jump(block, &[]);
            return Ok(());
        }
        let regs = self.regs;
        let edge = crate::liveness::Edge { target: target.pc, moves: &target.moves, payload: None };
        let movable: Vec<bool> = moves.iter().map(|(_, source)| self.live.movable(&edge, *source)).collect();
        // Снимок источников во временные слоты: перемещаемые — словами (источник
        // становится `Undef`), остальные — копией; затем запись назначений.
        let size = (moves.len() * SLOT as usize) as u32;
        let temporary = self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, 3));
        let temp = self.b.ins().stack_addr(types::I64, temporary, 0);
        for (index, (_, source)) in moves.iter().enumerate() {
            if movable[index] {
                self.copy_words(regs, offset(*source), temp, index as i32 * SLOT);
                self.store8(self.layout.undef, regs, offset(*source));
            } else {
                self.clone_into(regs, offset(*source), temp, index as i32 * SLOT);
            }
        }
        for (index, (dst, _)) in moves.iter().enumerate() {
            self.release(regs, offset(*dst));
            self.copy_words(temp, index as i32 * SLOT, regs, offset(*dst));
        }
        self.b.ins().jump(block, &[]);
        Ok(())
    }

    /// Ветвление помощником: `OK` — первая цель, `SECOND` — вторая.
    fn branching(&mut self, pc: usize, first: u32, second: u32) -> Result<(), String> {
        let first = self.block(first)?;
        let second = self.block(second)?;
        let status = self.eval(h_branch as *const () as usize, pc);
        let not_first = self.b.create_block();
        self.b.ins().brif(status, not_first, &[], first, &[]);
        self.b.switch_to_block(not_first);
        let is_second = self.b.ins().icmp_imm_s(IntCC::Equal, status, i64::from(SECOND));
        let (failed, param) = self.failure_entry();
        self.b.ins().brif(is_second, second, &[], failed, &[BlockArg::Value(status)]);
        self.fill_failure(failed, param, pc)
    }

    /// Обычная функция: прямой вызов без кадра интерпретатора.
    fn direct(&self, callee: usize) -> bool {
        let function = &self.program.functions[callee];
        !function.is_async && function.native.is_none() && function.intrinsic.is_none()
    }

    fn direct_call(&mut self, pc: usize, dst: Option<Register>, callee: u32, args: &[Register]) -> Result<(), String> {
        let program = self.program;
        let target = &program.functions[callee as usize];
        if args.len() != target.params.len() {
            return self.generic(pc);
        }
        self.call_into(pc, dst, Callee::Static(callee), &target.params, target.registers.len(), args)
    }

    /// Вызов обычной функции через таблицу кода: регистры вызываемой — в
    /// кадре, аргументы перемещаются при последнем использовании, без кода
    /// или без запаса стека — интерпретатор.
    fn call_into(&mut self, pc: usize, dst: Option<Register>, callee: Callee, params: &[Register], count: usize, args: &[Register]) -> Result<(), String> {
        let area = self.area_slot(count);
        let regs = self.regs;
        let base = self.b.ins().stack_addr(types::I64, area, 0);
        for register in 0..count {
            self.store8(self.layout.undef, base, register as i32 * SLOT);
        }
        for (param, arg) in params.iter().zip(args) {
            let once = args.iter().filter(|other| *other == arg).count() == 1;
            if once && !self.live.live_after(pc, *arg) {
                // Последнее использование аргумента: перемещение без копии.
                self.copy_words(regs, offset(*arg), base, offset(*param));
                self.store8(self.layout.undef, regs, offset(*arg));
            } else {
                self.clone_into(regs, offset(*arg), base, offset(*param));
            }
        }
        self.invoke_area(pc, dst, callee, base, count)
    }

    /// Вызов над подготовленными регистрами `base` (`count` слотов): код из
    /// таблицы или интерпретатор, освобождение регистров, статус и результат.
    fn invoke_area(&mut self, pc: usize, dst: Option<Register>, callee: Callee, base: ir::Value, count: usize) -> Result<(), String> {
        let result = match self.result {
            Some(slot) => slot,
            None => {
                let slot = self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, SLOT as u32, 3));
                self.result = Some(slot);
                slot
            }
        };
        let pointer = types::I64;
        let regs = self.regs;
        let out = self.b.ins().stack_addr(pointer, result, 0);
        self.store8(self.layout.undef, out, 0);
        // Запас стека и код вызываемой.
        let ctx = self.ctx;
        let sp = self.b.ins().get_stack_pointer(pointer);
        let floor_address = self.b.ins().load(pointer, flags(), ctx, 8);
        let floor = self.b.ins().load(pointer, flags(), floor_address, 0);
        let known = self.b.ins().icmp_imm_s(IntCC::NotEqual, floor, 0);
        let limit = self.b.ins().iadd_imm_s(floor, MARGIN as i64);
        let room = self.b.ins().icmp(IntCC::UnsignedGreaterThan, sp, limit);
        let allowed = self.b.ins().band(known, room);
        let table = self.b.ins().load(pointer, flags(), ctx, 0);
        let code = match callee {
            Callee::Static(index) => self.b.ins().load(pointer, flags(), table, index as i32 * 8),
            Callee::Dynamic(index) => {
                let wide = self.b.ins().uextend(pointer, index);
                let scaled = self.b.ins().ishl_imm_u(wide, 3);
                let entry = self.b.ins().iadd(table, scaled);
                self.b.ins().load(pointer, flags(), entry, 0)
            }
        };
        let compiled = self.b.ins().icmp_imm_s(IntCC::NotEqual, code, 0);
        let fast_ok = self.b.ins().band(allowed, compiled);
        let fast = self.b.create_block();
        let slow = self.b.create_block();
        let join = self.b.create_block();
        let status = self.b.append_block_param(join, types::I32);
        self.b.ins().brif(fast_ok, fast, &[], slow, &[]);
        self.b.switch_to_block(fast);
        let zero = self.i32(0);
        let inst = self.b.ins().call_indirect(self.sigs.code, code, &[ctx, base, out, zero]);
        let fast_status = self.b.inst_results(inst)[0];
        self.b.ins().jump(join, &[BlockArg::Value(fast_status)]);
        self.b.switch_to_block(slow);
        let callee_value = match callee {
            Callee::Static(index) => self.i32(index),
            Callee::Dynamic(index) => index,
        };
        let slow_status = self.helper(h_call_slow as *const () as usize, self.sigs.slow, &[ctx, base, out, callee_value]).expect("status");
        self.b.ins().jump(join, &[BlockArg::Value(slow_status)]);
        self.b.switch_to_block(join);
        // Регистры вызываемой освобождаются после вызова.
        if count <= INLINE_RELEASE {
            for register in 0..count {
                self.release(base, register as i32 * SLOT);
            }
        } else {
            let length = self.b.ins().iconst(types::I64, count as i64);
            self.helper(h_drop_range as *const () as usize, self.sigs.clone, &[base, length]);
        }
        self.check(status, pc)?;
        match dst {
            Some(dst) => {
                self.release(regs, offset(dst));
                self.copy_words(out, 0, regs, offset(dst));
            }
            None => self.release(out, 0),
        }
        Ok(())
    }

    /// `call.fn`: помощник кладёт захваты и аргументы в регистры вызываемой
    /// (область на `closure_registers` слотов) и возвращает цель; вызов —
    /// как прямой.
    fn closure_call(&mut self, pc: usize, dst: Option<Register>, callee: Register, args: &[Register]) -> Result<bool, String> {
        let program = self.program;
        let count = closure_registers(program);
        if count == 0 || args.contains(&callee) {
            return Ok(false);
        }
        let area = self.area_slot(count);
        let base = self.b.ins().stack_addr(types::I64, area, 0);
        for register in 0..count {
            self.store8(self.layout.undef, base, register as i32 * SLOT);
        }
        let mask = self.dying_mask(pc, args);
        let (func, at) = (self.i32(self.index), self.i32(pc as u32));
        let mask = self.b.ins().iconst(types::I64, mask as i64);
        let (ctx, regs) = (self.ctx, self.regs);
        let target = self.helper(h_fn_frame as *const () as usize, self.sigs.frame, &[ctx, regs, func, at, base, mask]).expect("target");
        let missing = self.b.ins().icmp_imm_s(IntCC::Equal, target, -1);
        let fast = self.b.create_block();
        let slow = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().brif(missing, slow, &[], fast, &[]);
        self.b.switch_to_block(fast);
        self.invoke_area(pc, dst, Callee::Dynamic(target), base, count)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        let status = self.eval(h_call_dyn as *const () as usize, pc);
        self.check(status, pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(true)
    }

    /// Слот регистров вызываемой не меньше `count` регистров.
    fn area_slot(&mut self, count: usize) -> ir::StackSlot {
        let size = (count.max(1) * SLOT as usize) as u32;
        match self.area {
            Some(slot) if self.b.func.sized_stack_slots[slot].size >= size => slot,
            _ => {
                let slot = self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size.max(SLOT as u32 * 64), 3));
                self.area = Some(slot);
                slot
            }
        }
    }

    /// `call.role`: реализации слота известны из таблиц программы; помощник
    /// только находит цель по receiver, вызов — как прямой.
    fn role_call(&mut self, pc: usize, dst: Option<Register>, role: u32, slot: u32, receiver: Register, args: &[Register]) -> Result<bool, String> {
        let program = self.program;
        let candidates: Vec<&Function> = program
            .objects
            .iter()
            .flat_map(|object| &object.does)
            .filter(|witness| witness.role.0 == role)
            .filter_map(|witness| witness.slots.get(slot as usize))
            .map(|callee| &program.functions[callee.0 as usize])
            .collect();
        let arity = args.len() + 1;
        let regular = |function: &&Function| {
            plain(function)
                && function.params.len() == arity
                && function.params.iter().enumerate().all(|(index, param)| param.0 as usize == index)
        };
        if candidates.is_empty() || !candidates.iter().all(regular) {
            return Ok(false);
        }
        let count = candidates.iter().map(|function| function.registers.len()).max().unwrap_or(0);
        if count > 512 {
            return Ok(false);
        }
        let target = match self.layout.object.filter(|object| object.declaration >= 0) {
            Some(layout) => self.role_dispatch(pc, layout, role, slot, receiver),
            None => self.eval(h_role_target as *const () as usize, pc),
        };
        let missing = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, target, 0);
        let fast = self.b.create_block();
        let slow = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().brif(missing, slow, &[], fast, &[]);
        self.b.switch_to_block(fast);
        let params: Vec<Register> = (0..arity as u32).map(Register).collect();
        let mut all = vec![receiver];
        all.extend_from_slice(args);
        self.call_into(pc, dst, Callee::Dynamic(target), &params, count, &all)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(slow);
        let status = self.eval(h_call_dyn as *const () as usize, pc);
        self.check(status, pc)?;
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(true)
    }
}
