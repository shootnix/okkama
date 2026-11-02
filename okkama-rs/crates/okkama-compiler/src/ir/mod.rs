//! OIR-1 — промежуточное представление программы (`docs/compiler/OIR-1.md`).
//!
//! Модуль содержит функции в форме типизированного SSA: у каждого значения
//! есть ровно одно определение и статический тип, блоки имеют параметры и
//! заканчиваются ровно одним терминатором. Значения и блоки функции
//! пронумерованы индексами; вход функции — блок 0, его значения — параметры
//! функции.
//!
//! [`verify`] проверяет правила §17 спецификации, текстовая форма (§18)
//! выводится через `Display` у [`Module`].

pub mod cfg;
pub mod hoist;
pub mod regions;
mod print;
pub mod recursion;
pub mod verify;

pub use verify::{VerifyError, verify};

use crate::source::{SourceFile, Span};

/// Статический тип значения OIR (§3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// Целое произвольной точности.
    Int,
    Bool,
    /// Целое 0..255.
    Byte,
    /// IEEE 754 binary64; неконечные значения не появляются (trap).
    Float,
    /// Точная несократимая дробь с положительным знаменателем.
    Rat,
    /// Unicode scalar value.
    Char,
    /// Неизменяемая строка Unicode.
    Str,
    /// Конкретная ссылка на объект модуля.
    Object(ObjectId),
    /// Результат successor-действия: обязательство использования (§8.4).
    Successor(ObjectId),
    /// Ограниченная ссылка роли (§9).
    Role(RoleId),
    /// Значение enum модуля.
    Enum(EnumId),
    /// Служебный: активный отказ (§6).
    Failure,
    /// Неизменяемый массив (§12.1).
    Array(Box<Type>),
    /// Элемент нетегированной пустой коллекции: `[]` без целевого типа.
    Never,
    /// Служебный: линейный сборщик массива (§12.2).
    Builder(Box<Type>),
    /// Неизменяемый словарь (§12.3): ключ `int`, `bool` или `str`;
    /// `map<never,never>` — нетегированный `[:]`.
    Map(Box<Type>, Box<Type>),
    /// Служебный: линейный сборщик словаря.
    MapBuilder(Box<Type>, Box<Type>),
    /// Служебный: регион владения лексической области (§7).
    Region,
    /// Непрозрачный native object (§14.1): создают и читают только natives.
    Handle(HandleId),
    /// Непрозрачный box значения (§3): распаковки нет.
    Any,
    /// Диапазон `a..b` / `a..=b` (§12.4).
    Range,
    /// Значение-функция `fn(T…) -> R` (§11): замыкание или связанный action.
    Fn(Box<FnSig>),
    /// `future<R>` (§13): исход async-функции (`future<void>` — без значения).
    Future(Box<ResultType>),
    /// Capability `Owner::Cap` (§8.5): владелец и вычисляемые поля.
    Cap(CapId),
}

/// Закрытый перечень intrinsics `okkama-rs` (§14.2): действия объектов std
/// `async/*`, чьё состояние держит рантайм.
pub const INTRINSICS: [&str; 68] = [
    "okkama.timeout.Within.result",
    "okkama.timeout.Before.result",
    "okkama.scope.FutureScope.result",
    "okkama.lock.Lock.new",
    "okkama.lock.Lock.guard",
    "okkama.lock.Lock.locked",
    "okkama.lock.LockGuard.closed",
    "okkama.lock.LockGuard.close",
    "okkama.semaphore.Semaphore.permit",
    "okkama.semaphore.Semaphore.permits",
    "okkama.semaphore.Permit.closed",
    "okkama.semaphore.Permit.close",
    "okkama.barrier.Barrier.wait",
    "okkama.barrier.Barrier.state",
    "okkama.barrier.Barrier.abort",
    "okkama.barrier.Barrier.reset",
    "okkama.barrier.BarrierState.parties",
    "okkama.barrier.BarrierState.waiters",
    "okkama.barrier.BarrierState.generation",
    "okkama.barrier.BarrierState.broken",
    "okkama.array.ordering",
    "okkama.test.ok",
    "okkama.test.is",
    "okkama.test.isnt",
    "okkama.test.fail",
    "okkama.fake.storage.set",
    "okkama.fake.storage.get",
    "okkama.fake.storage.has",
    "okkama.fake.storage.remove",
    "okkama.fake.storage.clear",
    "okkama.fake.storage.size",
    "okkama.channel.Channel.new_capacity",
    "okkama.channel.Channel.sending",
    "okkama.channel.Channel.receiving",
    "okkama.channel.Sending.send",
    "okkama.channel.Sending.close",
    "okkama.channel.Receiving.item",
    "okkama.channel.Receiving.close",
    "okkama.event.Event.new",
    "okkama.event.Event.new_ready",
    "okkama.event.Event.wait",
    "okkama.event.Event.set",
    "okkama.event.Event.ready",
    "okkama.atomic.Atomic.new_value",
    "okkama.atomic.Atomic.value",
    "okkama.atomic.Atomic.replace",
    "okkama.atomic.Atomic.compare_and_replace",
    "okkama.atomic.Atomic.closed",
    "okkama.atomic.Atomic.close",
    "okkama.cancellation.Cancellation.new",
    "okkama.cancellation.Cancellation.cancelling",
    "okkama.cancellation.Cancellation.cancel",
    "okkama.cancellation.CancellationToken.requested",
    "okkama.cancellation.CancellationToken.check",
    "okkama.cancellation.CancellationToken.wait",
    "okkama.scope.FutureGroup.new",
    "okkama.scope.FutureGroup.start",
    "okkama.scope.FutureGroup.active",
    "okkama.scope.FutureGroup.cancel",
    "okkama.scope.FutureGroup.wait",
    "okkama.collect.AllFutures.results",
    "okkama.collect.Raced.result",
    "okkama.collect.FirstSuccessful.result",
    "okkama.collect.FutureCompletions.remaining_count",
    "okkama.collect.FutureCompletions.completion",
    "okkama.channel.Sending.new",
    "okkama.channel.Receiving.new",
    "okkama.cancellation.CancellationToken.new",
];

/// Наблюдение и отмена Future (§13.2).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FutOp {
    Cancel,
    Done,
    Cancelled,
}

impl FutOp {
    pub fn mnemonic(self) -> &'static str {
        match self {
            FutOp::Cancel => "fut.cancel",
            FutOp::Done => "fut.done",
            FutOp::Cancelled => "fut.cancelled",
        }
    }
}

/// Сигнатура значения-функции: параметры и результат (типы точные).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnSig {
    pub params: Vec<Type>,
    pub result: ResultType,
}

impl Type {
    /// Тип значения после погашения обязательства `^C` → `C`.
    pub fn taken(&self) -> Type {
        match self {
            Type::Successor(object) => Type::Object(*object),
            other => other.clone(),
        }
    }

    /// Линейный служебный сборщик (`builder`, `mbuilder`).
    pub fn is_builder(&self) -> bool {
        matches!(self, Type::Builder(_) | Type::MapBuilder(..))
    }

    /// Допустимый тип ключа словаря.
    pub fn is_map_key(&self) -> bool {
        matches!(
            self,
            Type::Int
                | Type::Bool
                | Type::Byte
                | Type::Char
                | Type::Float
                | Type::Str
                | Type::Enum(_)
        )
    }
}

/// Результат функции: значение или `void`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResultType {
    Value(Type),
    Void,
}

pub use okkama_runtime_core::IntValue;

/// Модуль программы. Импорты пока сохраняются как ссылки без загрузки файлов.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    /// `v1.0.0`; пусто — модуль без версии.
    pub version: String,
    pub imports: Vec<Import>,
    pub enums: Vec<Enum>,
    pub roles: Vec<Role>,
    pub objects: Vec<Object>,
    /// Native objects (`native object H`, §14.1).
    pub handles: Vec<Handle>,
    /// Caps объектов (§8.5).
    pub caps: Vec<Cap>,
    /// Тестовый профиль (§15): тесты корневого модуля (имя и функция);
    /// `None` — production-модуль.
    pub tests: Option<Vec<(String, FuncId)>>,
    pub functions: Vec<Function>,
    /// Индекс функции-точки входа в `functions`.
    pub entry: Option<FuncId>,
    pub span: Span,
    pub debug: DebugInfo,
}

/// Отладочные сведения модуля: исходник каждой функции — для мест ошибок
/// исполнения. Это не часть программы: печать OIR и движки их не читают.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    /// Исходники модулей программы по номеру модуля (`None` — неизвестен).
    pub sources: Vec<Option<std::sync::Arc<SourceFile>>>,
    /// Номер модуля в `sources` для каждой функции `functions` (пусто — неизвестны).
    pub function_modules: Vec<usize>,
    /// Первый модуль корня (файлов программы); модули до него — импорты.
    pub first_root: usize,
}

impl Module {
    /// Исходник функции `func`, если он известен.
    pub fn source_of(&self, func: FuncId) -> Option<&SourceFile> {
        let module = *self.debug.function_modules.get(func.0)?;
        self.debug.sources.get(module)?.as_deref()
    }

    /// Функция `func` из файла самой программы, а не из импорта.
    pub fn in_root(&self, func: FuncId) -> bool {
        self.debug
            .function_modules
            .get(func.0)
            .is_some_and(|module| *module >= self.debug.first_root)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: String,
    pub alias: Option<String>,
    pub take: Vec<String>,
    pub span: Span,
}

/// Объявление объекта (§8.1): поля в порядке объявления.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub name: String,
    pub fields: Vec<Field>,
    /// Таблицы свидетельства ролей, которые объект делает (`does`).
    pub does: Vec<Witness>,
    /// Функция `destroy`: `[destroy C]`, `(self) -> void` (§7).
    pub destroy: Option<FuncId>,
    /// Системный объект рантайма (`NativeFailure`, §9): значения создаёт
    /// только рантайм, класс даёт backend; actions — чтения полей.
    pub builtin: bool,
    pub span: Span,
}

impl Object {
    /// Поля системного `NativeFailure`, все `str`, в этом порядке.
    pub const NATIVE_FAILURE_FIELDS: [&'static str; 4] = ["domain", "code", "operation", "message"];

    pub fn is_native_failure(&self) -> bool {
        self.builtin && self.name == "NativeFailure"
    }
}

/// Реализация роли объектом: функция для каждого слота роли по порядку.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness {
    pub role: RoleId,
    pub slots: Vec<FuncId>,
}

/// Объявление роли: слоты — сигнатуры actions в порядке объявления.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub name: String,
    pub methods: Vec<RoleMethod>,
    /// Системная роль (`Error`), доступная без импорта.
    pub builtin: bool,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleMethod {
    pub name: String,
    pub params: Vec<Type>,
    pub result: ResultType,
}

impl Role {
    /// Системная роль `Error` с действием `text() -> str`.
    pub fn is_error(&self) -> bool {
        self.builtin && self.name == "Error"
    }

    /// Имя слота: `name`, а у перегруженного имени — `name#k` по порядку.
    pub fn slot_name(&self, slot: usize) -> String {
        let method = &self.methods[slot];
        let same: Vec<usize> = (0..self.methods.len())
            .filter(|index| self.methods[*index].name == method.name)
            .collect();
        if same.len() == 1 {
            method.name.clone()
        } else {
            let position = same.iter().position(|index| *index == slot).unwrap_or(0);
            format!("{}#{position}", method.name)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    /// Функция `[default C.f]`, вычисляющая значение по умолчанию.
    pub default: Option<FuncId>,
}

impl Module {
    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(id.0)
    }

    pub fn role(&self, id: RoleId) -> Option<&Role> {
        self.roles.get(id.0)
    }

    pub fn enum_decl(&self, id: EnumId) -> Option<&Enum> {
        self.enums.get(id.0)
    }

    pub fn handle(&self, id: HandleId) -> Option<&Handle> {
        self.handles.get(id.0)
    }

    pub fn cap(&self, id: CapId) -> Option<&Cap> {
        self.caps.get(id.0)
    }

    /// Делает ли объект роль (по объявлению `does`).
    pub fn does(&self, object: ObjectId, role: RoleId) -> bool {
        self.object(object)
            .is_some_and(|object| object.does.iter().any(|witness| witness.role == role))
    }

    /// Можно ли бросить значение этого типа: объект, делающий `Error`, или роль `Error`.
    pub fn throwable(&self, ty: &Type) -> bool {
        match ty {
            Type::Object(object) => self.object(*object).is_some_and(|declared| {
                declared
                    .does
                    .iter()
                    .any(|witness| self.role(witness.role).is_some_and(Role::is_error))
            }),
            Type::Role(role) => self.role(*role).is_some_and(Role::is_error),
            _ => false,
        }
    }

    pub fn function(&self, id: FuncId) -> Option<&Function> {
        self.functions.get(id.0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnumId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandleId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CapId(pub usize);

/// `cap Owner::Name`: поля — функции над владельцем (`[method Owner]`),
/// каждое чтение вычисляет заново.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cap {
    pub name: String,
    pub owner: ObjectId,
    pub fields: Vec<(String, FuncId)>,
    pub span: Span,
}

/// Native object модуля: только имя; значения создаёт и читает рантайм.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handle {
    pub name: String,
    pub span: Span,
}

/// Объявление enum (§3): case в порядке объявления — это и их порядок (R06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enum {
    pub name: String,
    pub cases: Vec<String>,
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FuncId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub usize);

/// Атрибуты функции (§5.1).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Attr {
    Entry,
    /// Action объекта; первый параметр — `self`.
    Method(ObjectId),
    /// Конструктор: может выполнять `obj.new`.
    Ctor(ObjectId),
    /// Successor-действие: результат `^C`, может выполнять `obj.next`.
    Successor(ObjectId),
    /// Default поля объекта с данным индексом.
    Default(ObjectId, usize),
    /// Блок `destroy` объекта: вызывает только рантайм при закрытии региона.
    Destroy(ObjectId),
    /// `native`: тела нет, функцию с этим символом даёт рантайм backend
    /// (`runtime/stdio::stdout_write`, §14.1). Отказ provider — throwing.
    Extern(String),
    /// Вызов разрешён только функциям того же владельца.
    Private,
    /// Intrinsic (§14.2): тела нет, его даёт рантайм. `objects` — объекты
    /// модуля, которые рантайм создаёт (результаты, Error std); `helpers` —
    /// функции программы, которые рантайм вызывает (действия значений
    /// receiver: `limit.seconds()`, `clock.wait(…)`).
    Intrinsic {
        name: String,
        objects: Vec<ObjectId>,
        helpers: Vec<FuncId>,
    },
    /// Async-функция (§13.1): `call` даёт холодный `future<R>`, тело
    /// исполняется при первом `await` (или `fut.spawn`).
    Async,
    /// Тело анонимного action (§11.1): первые `captures` параметров —
    /// захваты; `owner` — объект, в чьём action оно написано (читает его поля).
    Lambda {
        captures: usize,
        owner: Option<ObjectId>,
    },
}

impl Function {
    pub fn is_async(&self) -> bool {
        self.attrs.contains(&Attr::Async)
    }

    /// Место инструкции `pc` в нумерации, общей с Sparrow: инструкции блоков
    /// по порядку, терминатор блока — после его инструкций.
    pub fn span_at(&self, pc: u32) -> Option<Span> {
        let mut rest = pc as usize;
        for block in &self.blocks {
            match rest.cmp(&block.insts.len()) {
                std::cmp::Ordering::Less => return Some(block.insts[rest].span),
                std::cmp::Ordering::Equal => return Some(block.term.span),
                std::cmp::Ordering::Greater => rest -= block.insts.len() + 1,
            }
        }
        None
    }

    /// Результат `call` функции: у async — `future<R>`.
    pub fn call_result(&self) -> ResultType {
        if self.is_async() {
            ResultType::Value(Type::Future(Box::new(self.result.clone())))
        } else {
            self.result.clone()
        }
    }

    /// Объект, которому принадлежит функция (метод, конструктор, successor, default).
    pub fn owner(&self) -> Option<ObjectId> {
        self.attrs.iter().find_map(|attr| match attr {
            Attr::Method(object)
            | Attr::Ctor(object)
            | Attr::Successor(object)
            | Attr::Default(object, _)
            | Attr::Destroy(object) => Some(*object),
            Attr::Lambda { owner, .. } => *owner,
            Attr::Entry
            | Attr::Private
            | Attr::Extern(_)
            | Attr::Async
            | Attr::Intrinsic { .. } => None,
        })
    }

    /// Символ рантайма, если функция — `native`.
    /// Имя intrinsic и объекты, которые рантайм создаёт.
    pub fn intrinsic(&self) -> Option<(&str, &[ObjectId])> {
        self.attrs.iter().find_map(|attr| match attr {
            Attr::Intrinsic { name, objects, .. } => Some((name.as_str(), objects.as_slice())),
            _ => None,
        })
    }

    /// Функции программы, которые вызывает intrinsic (пусто у остальных).
    pub fn intrinsic_helpers(&self) -> &[FuncId] {
        self.attrs
            .iter()
            .find_map(|attr| match attr {
                Attr::Intrinsic { helpers, .. } => Some(helpers.as_slice()),
                _ => None,
            })
            .unwrap_or(&[])
    }

    pub fn extern_symbol(&self) -> Option<&str> {
        self.attrs.iter().find_map(|attr| match attr {
            Attr::Extern(symbol) => Some(symbol.as_str()),
            _ => None,
        })
    }

    pub fn is_private(&self) -> bool {
        self.attrs.contains(&Attr::Private)
    }
}

/// Функция в форме SSA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    /// Символ без `@`, например `main` или `Purse.total`.
    pub symbol: String,
    /// Значения-параметры; определены на входе блока 0.
    pub params: Vec<ValueId>,
    pub result: ResultType,
    pub attrs: Vec<Attr>,
    /// Тип каждого значения функции по индексу `ValueId`.
    pub values: Vec<Type>,
    /// Блок 0 — вход функции.
    pub blocks: Vec<Block>,
    pub span: Span,
}

impl Function {
    pub fn value_type(&self, value: ValueId) -> Option<&Type> {
        self.values.get(value.0)
    }
}

/// Базовый блок: параметры, инструкции и обязательный терминатор.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub params: Vec<ValueId>,
    /// Цель отказа любой инструкции блока (§6.2): аргументы определены до
    /// начала блока, значение `failure` добавляется последним параметром цели.
    pub unwind: Option<Target>,
    pub insts: Vec<Inst>,
    pub term: Terminator,
}

/// Переход к блоку с аргументами для его параметров.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub block: BlockId,
    pub args: Vec<ValueId>,
}

impl Target {
    pub fn new(block: BlockId) -> Self {
        Self {
            block,
            args: Vec::new(),
        }
    }
}

/// Инструкция с местоположением исходной конструкции.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inst {
    /// Определяемое значение; у инструкций без результата — `None`.
    pub result: Option<ValueId>,
    pub kind: InstKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Constant {
    Int(IntValue),
    Bool(bool),
    Byte(u8),
    /// Биты конечного binary64 (`f64::to_bits`): так константа сравнима.
    Float(u64),
    Char(char),
    Str(String),
}

impl Constant {
    pub fn ty(&self) -> Type {
        match self {
            Constant::Int(_) => Type::Int,
            Constant::Bool(_) => Type::Bool,
            Constant::Byte(_) => Type::Byte,
            Constant::Float(_) => Type::Float,
            Constant::Char(_) => Type::Char,
            Constant::Str(_) => Type::Str,
        }
    }
}

/// Операции над `float` (§16.2): неконечный результат, делитель 0 — trap.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FloatOp {
    Add,
    Sub,
    Mul,
    Div,
    /// Остаток со знаком делителя, как `%` над `int`.
    Mod,
    Pow,
    Neg,
    Abs,
    Sqrt,
}

impl FloatOp {
    pub fn mnemonic(self) -> &'static str {
        match self {
            FloatOp::Add => "float.add",
            FloatOp::Sub => "float.sub",
            FloatOp::Mul => "float.mul",
            FloatOp::Div => "float.div",
            FloatOp::Mod => "float.mod",
            FloatOp::Pow => "float.pow",
            FloatOp::Neg => "float.neg",
            FloatOp::Abs => "float.abs",
            FloatOp::Sqrt => "float.sqrt",
        }
    }

    /// Число операндов.
    pub fn arity(self) -> usize {
        match self {
            FloatOp::Neg | FloatOp::Abs | FloatOp::Sqrt => 1,
            _ => 2,
        }
    }
}

/// Операции над `rat` (§16.2): `new` — из двух `int` (знаменатель 0 — trap),
/// остальные — над `rat`; делитель 0 — trap.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RatOp {
    New,
    Add,
    Sub,
    Mul,
    Div,
    /// Остаток со знаком делителя.
    Mod,
    Neg,
    Abs,
}

impl RatOp {
    pub fn mnemonic(self) -> &'static str {
        match self {
            RatOp::New => "rat.new",
            RatOp::Add => "rat.add",
            RatOp::Sub => "rat.sub",
            RatOp::Mul => "rat.mul",
            RatOp::Div => "rat.div",
            RatOp::Mod => "rat.mod",
            RatOp::Neg => "rat.neg",
            RatOp::Abs => "rat.abs",
        }
    }

    pub fn arity(self) -> usize {
        match self {
            RatOp::Neg | RatOp::Abs => 1,
            _ => 2,
        }
    }
}

/// Текст `float` по правилу Go `strconv.FormatFloat(x, 'g', -1, 64)` (D-032):
/// кратчайшие цифры, экспонента при порядке < -4 или ≥ 6 (`1e+06`, `2.5e-07`),
/// знак нуля сохраняется (`-0`).
pub fn format_float(value: f64) -> String {
    okkama_runtime_core::float::text(value)
}

/// Битовые действия над словом `int` (signed-64) или `byte` (8 бит), §16.2.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BitOp {
    BitCount,
    LeadingZeros,
    TrailingZeros,
    /// Поворот на счётчик `int` в `0..ширина-1`.
    RotateLeft,
    RotateRight,
    ReverseBits,
    SwapBytes,
}

impl BitOp {
    pub fn name(self) -> &'static str {
        match self {
            BitOp::BitCount => "bit_count",
            BitOp::LeadingZeros => "leading_zeros",
            BitOp::TrailingZeros => "trailing_zeros",
            BitOp::RotateLeft => "rotl",
            BitOp::RotateRight => "rotr",
            BitOp::ReverseBits => "reverse_bits",
            BitOp::SwapBytes => "swap_bytes",
        }
    }

    /// Действие Okkama с этим смыслом (`rotate_left`, а не `rotl`).
    pub fn from_action(name: &str) -> Option<Self> {
        Some(match name {
            "bit_count" => BitOp::BitCount,
            "leading_zeros" => BitOp::LeadingZeros,
            "trailing_zeros" => BitOp::TrailingZeros,
            "rotate_left" => BitOp::RotateLeft,
            "rotate_right" => BitOp::RotateRight,
            "reverse_bits" => BitOp::ReverseBits,
            "swap_bytes" => BitOp::SwapBytes,
            _ => return None,
        })
    }

    /// Результат — число битов (`int`), а не слово исходного типа.
    pub fn counts(self) -> bool {
        matches!(
            self,
            BitOp::BitCount | BitOp::LeadingZeros | BitOp::TrailingZeros
        )
    }

    pub fn rotates(self) -> bool {
        matches!(self, BitOp::RotateLeft | BitOp::RotateRight)
    }
}

/// Бинарные операции над `int` (§16.2).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntBinaryOp {
    Add,
    Sub,
    Mul,
    /// Остаток усечённого деления (знак делимого, как `%` Go; D-031).
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
}

impl IntBinaryOp {
    pub fn mnemonic(self) -> &'static str {
        match self {
            IntBinaryOp::Add => "int.add",
            IntBinaryOp::Sub => "int.sub",
            IntBinaryOp::Mul => "int.mul",
            IntBinaryOp::Mod => "int.mod",
            IntBinaryOp::And => "int.and",
            IntBinaryOp::Or => "int.or",
            IntBinaryOp::Xor => "int.xor",
            IntBinaryOp::Shl => "int.shl",
            IntBinaryOp::Shr => "int.shr",
            IntBinaryOp::Ushr => "int.ushr",
        }
    }

    /// Мнемоника той же операции над `byte` (`byte.and`, `byte.shl`, …).
    pub fn byte_mnemonic(self) -> String {
        self.mnemonic().replacen("int.", "byte.", 1)
    }

    /// Класс отказа: `true`, если инструкция может дать программную ошибку.
    pub fn traps(self) -> bool {
        !matches!(self, IntBinaryOp::Add | IntBinaryOp::Sub | IntBinaryOp::Mul)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntUnaryOp {
    Neg,
    /// Побитовая инверсия слова signed-64.
    Not,
}

impl IntUnaryOp {
    pub fn mnemonic(self) -> &'static str {
        match self {
            IntUnaryOp::Neg => "int.neg",
            IntUnaryOp::Not => "int.not",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CompareOp {
    pub fn suffix(self) -> &'static str {
        match self {
            CompareOp::Eq => "eq",
            CompareOp::Ne => "ne",
            CompareOp::Lt => "lt",
            CompareOp::Le => "le",
            CompareOp::Gt => "gt",
            CompareOp::Ge => "ge",
        }
    }

    pub fn is_equality(self) -> bool {
        matches!(self, CompareOp::Eq | CompareOp::Ne)
    }
}

/// Операции инструкций (§16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstKind {
    Const(Constant),
    IntBinary {
        op: IntBinaryOp,
        left: ValueId,
        right: ValueId,
    },
    IntUnary {
        op: IntUnaryOp,
        operand: ValueId,
    },
    IntCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    BoolNot(ValueId),
    /// Только `Eq` и `Ne`.
    BoolCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    StrConcat {
        left: ValueId,
        right: ValueId,
    },
    /// Только `Eq` и `Ne`.
    StrCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// Новый объект: значения всех полей в порядке объявления.
    ObjNew {
        object: ObjectId,
        fields: Vec<ValueId>,
    },
    /// Связать поле объекта, чей `self` ушёл значением до конца конструктора
    /// (§11.3): только в конструкторе этого объекта.
    ObjInit {
        object: ValueId,
        field: usize,
        value: ValueId,
    },
    /// Чтение поля по индексу.
    ObjGet {
        object: ValueId,
        field: usize,
    },
    /// Новый объект из полей `object` с заменами (successor).
    ObjNext {
        object: ValueId,
        updates: Vec<(usize, ValueId)>,
    },
    /// Прямой вызов функции модуля; у void-функции нет результата.
    Call {
        func: FuncId,
        args: Vec<ValueId>,
    },
    /// Погасить обязательство `^C` и получить значение `C`.
    SuccTake(ValueId),
    /// Преобразование из таблицы §16.1; целевой тип — тип результата.
    Cvt(ValueId),
    /// Конкретный объект → ограниченная ссылка роли (роль — тип результата).
    RoleWrap(ValueId),
    /// `any.box`: значение в непрозрачный box `any`.
    AnyBox(ValueId),
    /// `fn.closure @f [%c…]`: значение-функция из `@f` и снимка захватов —
    /// лямбды (`[lambda]`) или action объекта с захваченным receiver
    /// (`fn.bind`).
    FnClosure {
        func: FuncId,
        captures: Vec<ValueId>,
    },
    /// `call.fn %f(%a…)`.
    CallFn {
        callee: ValueId,
        args: Vec<ValueId>,
    },
    /// `cap.make %owner` (§8.5): capability владельца; тип — у результата.
    CapMake(ValueId),
    /// `call.cap %k, field()`: вычислить поле cap заново.
    CallCap {
        cap: ValueId,
        field: usize,
    },
    /// `await %fu` (§13.1): только в `[async]`; результат — `R` future<R>.
    Await(ValueId),
    /// `fut.spawn %fu`: запустить сейчас; результат — тот же Future.
    FutSpawn(ValueId),
    /// `worker.spawn %f(%a…)` (§18.1): синхронный action в отдельном
    /// процессе-воркере; результат — `future<R>` типа `%f`. `direct` — `%f`
    /// сам вход воркера (`spawn Worker(…).action(…)`); иначе `%f` — значение
    /// action, созданное `worker.bind`.
    WorkerSpawn {
        entry: ValueId,
        args: Vec<ValueId>,
        direct: bool,
    },
    /// `worker.bind %action, %entry, [%field…]`: bound action, которое
    /// `worker.spawn` исполнит в воркере: вход `%entry` заново конструирует
    /// receiver из значений его полей.
    WorkerBind {
        action: ValueId,
        entry: ValueId,
        fields: Vec<ValueId>,
    },
    /// `fut.cancel` / `fut.done` / `fut.cancelled` (§13.2).
    Fut {
        op: FutOp,
        future: ValueId,
    },
    /// Произвольное значение типа результата: заглушка поля на пути, где оно
    /// не связано; читать его можно только под проверкой признака связывания.
    Undef,
    /// Ссылка роли → ссылка другой роли (тип — из результата); если исходное
    /// значение её не делает — программная ошибка с текстом `message` (§9).
    RoleNarrow {
        value: ValueId,
        message: String,
    },
    /// Вызов слота роли на ограниченной ссылке.
    CallRole {
        receiver: ValueId,
        slot: usize,
        args: Vec<ValueId>,
    },
    /// `new` с причиной `handled`, если это новый типизированный отказ (§6.3).
    FailCaused {
        new: ValueId,
        handled: ValueId,
    },
    /// Массив из значений; тип элемента — из типа результата.
    ArrayNew(Vec<ValueId>),
    ArrayLen(ValueId),
    /// Элемент по индексу; вне границ — программная ошибка.
    ArrayGet {
        array: ValueId,
        index: ValueId,
    },
    /// `[start, end)` без clamp; нарушение границ — программная ошибка.
    ArraySlice {
        array: ValueId,
        start: ValueId,
        end: ValueId,
    },
    /// Склейка `array<str>` через разделитель.
    ArrayJoin {
        array: ValueId,
        separator: ValueId,
    },
    /// `array<never>` → `array<T>` (тип — из результата).
    Retag(ValueId),
    /// Пустой сборщик; тип элемента — из типа результата.
    BuilderNew,
    BuilderPush {
        builder: ValueId,
        value: ValueId,
    },
    BuilderAppend {
        builder: ValueId,
        array: ValueId,
    },
    /// Целые `start..end` (`..=` при `inclusive`); `start > end` — программная ошибка.
    BuilderAppendRange {
        builder: ValueId,
        start: ValueId,
        end: ValueId,
        inclusive: bool,
    },
    BuilderFinish(ValueId),
    /// Строковая операция §12.5; операнды — по виду.
    Str {
        op: StrOp,
        args: Vec<ValueId>,
    },
    /// Глубокое равенство одинаковых статических типов (§16.3), `Eq` или `Ne`.
    ValCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// `abs()` и `sign()` целого.
    IntAbs(ValueId),
    IntSign(ValueId),
    /// Битовые операции над `byte`: `and or xor` двух `byte`; `shl shr ushr`
    /// со счётчиком `int` в 0..7 (иначе программная ошибка).
    ByteBinary {
        op: IntBinaryOp,
        left: ValueId,
        right: ValueId,
    },
    ByteNot(ValueId),
    /// `int → byte`: вне 0..255 — программная ошибка; `context` — префикс
    /// диагностики (`@byte() argument 3`), как у Perl.
    ByteFrom {
        value: ValueId,
        context: Option<String>,
    },
    ByteCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// Только `Eq` и `Ne`.
    CharCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// Открыть регион области (§7).
    RegionEnter,
    /// Владеть свободными идентичностями, достижимыми из `value`.
    RegionAdopt {
        region: ValueId,
        value: ValueId,
    },
    /// Передать владение: идентичности, достижимые из `value` и
    /// принадлежащие `from`, переходят к `to` (поле строящегося объекта
    /// принадлежит корню конструктора, а не вложенной области).
    RegionTransfer {
        from: ValueId,
        to: ValueId,
        value: ValueId,
    },
    /// Учитывать свободные идентичности `value` без владения: если их не
    /// примет вызванное действие, регион уничтожит их при закрытии.
    RegionTrack {
        region: ValueId,
        value: ValueId,
    },
    /// Закрыть регион: уничтожить его идентичности (кроме достижимых из
    /// `keep` — они освобождаются); первый отказ destroy — основной.
    RegionLeave {
        region: ValueId,
        keep: Option<ValueId>,
    },
    /// Закрыть регион на пути отказа: Error из `failure` освобождается,
    /// отказы destroy становятся вторичными.
    RegionUnwind {
        region: ValueId,
        failure: ValueId,
    },
    /// Отказ cleanup `cleanup` при идущем отказе `primary`: вторичный.
    FailSecondary {
        primary: ValueId,
        cleanup: ValueId,
    },
    /// Case enum по индексу в объявлении.
    EnumCase {
        ty: EnumId,
        case: usize,
    },
    /// Только `Eq` и `Ne`; операнды — один enum.
    EnumCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// Операция над `float` (операнды — по `op.arity()`).
    Float {
        op: FloatOp,
        args: Vec<ValueId>,
    },
    FloatCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// Знак `float`: `int` −1, 0 или 1.
    FloatSign(ValueId),
    /// Операция над `rat` (операнды — по `op.arity()`).
    Rat {
        op: RatOp,
        args: Vec<ValueId>,
    },
    RatCompare {
        op: CompareOp,
        left: ValueId,
        right: ValueId,
    },
    /// Знак `rat`: `int` −1, 0 или 1.
    RatSign(ValueId),
    /// Числитель (`denominator == false`) или знаменатель `rat` — `str`.
    RatPart {
        value: ValueId,
        denominator: bool,
    },
    /// `range.new %a, %b, inclusive|exclusive`; начало больше конца — trap.
    RangeNew {
        start: ValueId,
        end: ValueId,
        inclusive: bool,
    },
    /// `range.start` / `range.end` (эксклюзивная верхняя граница).
    RangePart {
        range: ValueId,
        end: bool,
    },
    /// Битовое действие над `int` (`byte == false`) или `byte`; `count` —
    /// счётчик поворота.
    Bits {
        op: BitOp,
        byte: bool,
        value: ValueId,
        count: Option<ValueId>,
    },
    /// Словарь из пар; при повторе ключа побеждает последнее значение (R04).
    MapNew(Vec<(ValueId, ValueId)>),
    MapLen(ValueId),
    MapHas {
        map: ValueId,
        key: ValueId,
    },
    /// Значение по ключу; отсутствующий ключ — программная ошибка.
    MapGet {
        map: ValueId,
        key: ValueId,
    },
    /// Ключи в естественном порядке типа ключа (R06).
    MapKeys(ValueId),
    /// Значения в порядке ключей.
    MapValues(ValueId),
    /// `map<never,never>` → `map<K,V>` (тип — из результата).
    MapRetag(ValueId),
    MapBuilderNew,
    MapBuilderPut {
        builder: ValueId,
        key: ValueId,
        value: ValueId,
    },
    MapBuilderFinish(ValueId),
}

/// Строковые операции (§12.5): семантика Perl-интерпретатора по code points.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StrOp {
    /// `length()`: число code points.
    Len,
    /// `slice(start, end)` по code points, без clamp.
    Slice,
    /// `split(separator)`: буквальный; пустой разделитель — по code points.
    Split,
    /// `replace(target, replacement)`: все непересекающиеся вхождения.
    Replace,
    ReplaceFirst,
    /// `trim()`: крайние пробельные символы `\s` Perl.
    Trim,
    /// `chomp()`: один завершающий CRLF, LF или CR.
    Chomp,
    /// Строгий UTF-8 строки: `array<byte>` (строка как `ByteSource`, §15).
    Utf8,
}

impl StrOp {
    pub fn mnemonic(self) -> &'static str {
        match self {
            StrOp::Len => "str.len",
            StrOp::Slice => "str.slice",
            StrOp::Split => "str.split",
            StrOp::Replace => "str.replace",
            StrOp::ReplaceFirst => "str.replace_first",
            StrOp::Trim => "str.trim",
            StrOp::Chomp => "str.chomp",
            StrOp::Utf8 => "str.utf8",
        }
    }

    /// Число операндов, включая строку-receiver.
    pub fn arity(self) -> usize {
        match self {
            StrOp::Len | StrOp::Trim | StrOp::Chomp | StrOp::Utf8 => 1,
            StrOp::Split => 2,
            StrOp::Slice | StrOp::Replace | StrOp::ReplaceFirst => 3,
        }
    }
}

impl InstKind {
    /// Значения, которые читает инструкция, в порядке операндов.
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            InstKind::Const(_) => Vec::new(),
            InstKind::IntBinary { left, right, .. }
            | InstKind::IntCompare { left, right, .. }
            | InstKind::BoolCompare { left, right, .. }
            | InstKind::StrConcat { left, right }
            | InstKind::StrCompare { left, right, .. } => vec![*left, *right],
            InstKind::IntUnary { operand, .. }
            | InstKind::BoolNot(operand)
            | InstKind::SuccTake(operand)
            | InstKind::Cvt(operand)
            | InstKind::RoleWrap(operand)
            | InstKind::AnyBox(operand)
            | InstKind::RoleNarrow { value: operand, .. }
            | InstKind::ObjGet {
                object: operand, ..
            } => vec![*operand],
            InstKind::CallRole { receiver, args, .. } => std::iter::once(*receiver)
                .chain(args.iter().copied())
                .collect(),
            InstKind::FailCaused { new, handled } => vec![*new, *handled],
            InstKind::ArrayNew(values) => values.clone(),
            InstKind::ArrayLen(value)
            | InstKind::Retag(value)
            | InstKind::BuilderFinish(value)
            | InstKind::IntAbs(value)
            | InstKind::IntSign(value) => vec![*value],
            InstKind::ArrayGet { array, index } => vec![*array, *index],
            InstKind::ArraySlice { array, start, end } => vec![*array, *start, *end],
            InstKind::ArrayJoin { array, separator } => vec![*array, *separator],
            InstKind::BuilderNew => Vec::new(),
            InstKind::BuilderPush { builder, value } => vec![*builder, *value],
            InstKind::BuilderAppend { builder, array } => vec![*builder, *array],
            InstKind::BuilderAppendRange {
                builder,
                start,
                end,
                ..
            } => vec![*builder, *start, *end],
            InstKind::Str { args, .. } => args.clone(),
            InstKind::ValCompare { left, right, .. } => vec![*left, *right],
            InstKind::ObjNew { fields, .. } => fields.clone(),
            InstKind::ObjNext { object, updates } => std::iter::once(*object)
                .chain(updates.iter().map(|(_, value)| *value))
                .collect(),
            InstKind::Call { args, .. } => args.clone(),
            InstKind::ByteBinary { left, right, .. }
            | InstKind::ByteCompare { left, right, .. }
            | InstKind::CharCompare { left, right, .. }
            | InstKind::FloatCompare { left, right, .. }
            | InstKind::RatCompare { left, right, .. }
            | InstKind::EnumCompare { left, right, .. } => vec![*left, *right],
            InstKind::Float { args, .. } => args.clone(),
            InstKind::FloatSign(value) => vec![*value],
            InstKind::Rat { args, .. } => args.clone(),
            InstKind::RatSign(value) | InstKind::RatPart { value, .. } => vec![*value],
            InstKind::RangeNew { start, end, .. } => vec![*start, *end],
            InstKind::FnClosure { captures, .. } => captures.clone(),
            InstKind::Await(future) | InstKind::FutSpawn(future) | InstKind::Fut { future, .. } => {
                vec![*future]
            }
            InstKind::CapMake(owner) | InstKind::CallCap { cap: owner, .. } => vec![*owner],
            InstKind::ObjInit { object, value, .. } => vec![*object, *value],
            InstKind::CallFn { callee, args }
            | InstKind::WorkerSpawn {
                entry: callee,
                args,
                ..
            } => std::iter::once(*callee)
                .chain(args.iter().copied())
                .collect(),
            InstKind::WorkerBind {
                action,
                entry,
                fields,
            } => [*action, *entry]
                .into_iter()
                .chain(fields.iter().copied())
                .collect(),
            InstKind::RangePart { range, .. } => vec![*range],
            InstKind::EnumCase { .. } | InstKind::RegionEnter | InstKind::Undef => Vec::new(),
            InstKind::RegionAdopt { region, value } | InstKind::RegionTrack { region, value } => {
                vec![*region, *value]
            }
            InstKind::RegionTransfer { from, to, value } => vec![*from, *to, *value],
            InstKind::RegionLeave { region, keep } => std::iter::once(*region)
                .chain(keep.iter().copied())
                .collect(),
            InstKind::RegionUnwind { region, failure } => vec![*region, *failure],
            InstKind::FailSecondary { primary, cleanup } => vec![*primary, *cleanup],
            InstKind::ByteNot(value) | InstKind::ByteFrom { value, .. } => vec![*value],
            InstKind::Bits { value, count, .. } => std::iter::once(*value)
                .chain(count.iter().copied())
                .collect(),
            InstKind::MapNew(entries) => entries.iter().flat_map(|(k, v)| [*k, *v]).collect(),
            InstKind::MapLen(value)
            | InstKind::MapKeys(value)
            | InstKind::MapValues(value)
            | InstKind::MapRetag(value)
            | InstKind::MapBuilderFinish(value) => vec![*value],
            InstKind::MapHas { map, key } | InstKind::MapGet { map, key } => vec![*map, *key],
            InstKind::MapBuilderNew => Vec::new(),
            InstKind::MapBuilderPut {
                builder,
                key,
                value,
            } => vec![*builder, *key, *value],
        }
    }

    pub fn operands_mut(&mut self) -> Vec<&mut ValueId> {
        match self {
            InstKind::Const(_) => Vec::new(),
            InstKind::IntBinary { left, right, .. }
            | InstKind::IntCompare { left, right, .. }
            | InstKind::BoolCompare { left, right, .. }
            | InstKind::StrConcat { left, right }
            | InstKind::StrCompare { left, right, .. } => vec![left, right],
            InstKind::IntUnary { operand, .. }
            | InstKind::BoolNot(operand)
            | InstKind::SuccTake(operand)
            | InstKind::Cvt(operand)
            | InstKind::RoleWrap(operand)
            | InstKind::AnyBox(operand)
            | InstKind::RoleNarrow { value: operand, .. }
            | InstKind::ObjGet {
                object: operand, ..
            } => vec![operand],
            InstKind::CallRole { receiver, args, .. } => {
                std::iter::once(receiver).chain(args.iter_mut()).collect()
            }
            InstKind::FailCaused { new, handled } => vec![new, handled],
            InstKind::ArrayNew(values) => values.iter_mut().collect(),
            InstKind::ArrayLen(value)
            | InstKind::Retag(value)
            | InstKind::BuilderFinish(value)
            | InstKind::IntAbs(value)
            | InstKind::IntSign(value) => vec![value],
            InstKind::ArrayGet { array, index } => vec![array, index],
            InstKind::ArraySlice { array, start, end } => vec![array, start, end],
            InstKind::ArrayJoin { array, separator } => vec![array, separator],
            InstKind::BuilderNew => Vec::new(),
            InstKind::BuilderPush { builder, value } => vec![builder, value],
            InstKind::BuilderAppend { builder, array } => vec![builder, array],
            InstKind::BuilderAppendRange {
                builder,
                start,
                end,
                ..
            } => vec![builder, start, end],
            InstKind::Str { args, .. } => args.iter_mut().collect(),
            InstKind::ValCompare { left, right, .. } => vec![left, right],
            InstKind::ObjNew { fields, .. } => fields.iter_mut().collect(),
            InstKind::ObjNext { object, updates } => std::iter::once(object)
                .chain(updates.iter_mut().map(|(_, value)| value))
                .collect(),
            InstKind::Call { args, .. } => args.iter_mut().collect(),
            InstKind::ByteBinary { left, right, .. }
            | InstKind::ByteCompare { left, right, .. }
            | InstKind::CharCompare { left, right, .. }
            | InstKind::FloatCompare { left, right, .. }
            | InstKind::RatCompare { left, right, .. }
            | InstKind::EnumCompare { left, right, .. } => vec![left, right],
            InstKind::Float { args, .. } => args.iter_mut().collect(),
            InstKind::FloatSign(value) => vec![value],
            InstKind::Rat { args, .. } => args.iter_mut().collect(),
            InstKind::RatSign(value) | InstKind::RatPart { value, .. } => vec![value],
            InstKind::RangeNew { start, end, .. } => vec![start, end],
            InstKind::FnClosure { captures, .. } => captures.iter_mut().collect(),
            InstKind::Await(future) | InstKind::FutSpawn(future) | InstKind::Fut { future, .. } => {
                vec![future]
            }
            InstKind::CapMake(owner) | InstKind::CallCap { cap: owner, .. } => vec![owner],
            InstKind::ObjInit { object, value, .. } => vec![object, value],
            InstKind::CallFn { callee, args }
            | InstKind::WorkerSpawn {
                entry: callee,
                args,
                ..
            } => std::iter::once(callee).chain(args.iter_mut()).collect(),
            InstKind::WorkerBind {
                action,
                entry,
                fields,
            } => [action, entry]
                .into_iter()
                .chain(fields.iter_mut())
                .collect(),
            InstKind::RangePart { range, .. } => vec![range],
            InstKind::EnumCase { .. } | InstKind::RegionEnter | InstKind::Undef => Vec::new(),
            InstKind::RegionAdopt { region, value } | InstKind::RegionTrack { region, value } => {
                vec![region, value]
            }
            InstKind::RegionTransfer { from, to, value } => vec![from, to, value],
            InstKind::RegionLeave { region, keep } => {
                std::iter::once(region).chain(keep.iter_mut()).collect()
            }
            InstKind::RegionUnwind { region, failure } => vec![region, failure],
            InstKind::FailSecondary { primary, cleanup } => vec![primary, cleanup],
            InstKind::ByteNot(value) | InstKind::ByteFrom { value, .. } => vec![value],
            InstKind::Bits { value, count, .. } => {
                std::iter::once(value).chain(count.iter_mut()).collect()
            }
            InstKind::MapNew(entries) => entries.iter_mut().flat_map(|(k, v)| [k, v]).collect(),
            InstKind::MapLen(value)
            | InstKind::MapKeys(value)
            | InstKind::MapValues(value)
            | InstKind::MapRetag(value)
            | InstKind::MapBuilderFinish(value) => vec![value],
            InstKind::MapHas { map, key } | InstKind::MapGet { map, key } => vec![map, key],
            InstKind::MapBuilderNew => Vec::new(),
            InstKind::MapBuilderPut {
                builder,
                key,
                value,
            } => vec![builder, key, value],
        }
    }
}

/// Терминатор блока с местоположением.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terminator {
    pub kind: TermKind,
    pub span: Span,
}

/// Коды программных ошибок `trap` (§5.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TrapCode {
    /// Не выполнен `confirm` без `else`.
    Confirm,
    /// Начало диапазона больше конца.
    Range,
    /// `first()`/`last()` пустого массива.
    Empty,
    /// Поле не связано после тела конструктора.
    Unbound,
    /// Исполнение достигло `unreachable`-точки lowering.
    Unreachable,
}

impl TrapCode {
    pub fn name(self) -> &'static str {
        match self {
            TrapCode::Confirm => "confirm",
            TrapCode::Range => "range",
            TrapCode::Empty => "empty",
            TrapCode::Unbound => "unbound",
            TrapCode::Unreachable => "unreachable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermKind {
    Br(Target),
    Cond {
        condition: ValueId,
        then_target: Target,
        else_target: Target,
    },
    Ret(Option<ValueId>),
    /// Неперехватываемый отказ с точной строкой.
    Panic(ValueId),
    Trap {
        code: TrapCode,
        message: String,
    },
    Unreachable,
    /// Поднять типизированный отказ: объект или роль, делающие `Error`.
    Throw(ValueId),
    /// Продолжить распространение отказа `failure`.
    Raise(ValueId),
    /// Выбор catch (§6.4): при совпадении `handler` получает пойманное
    /// значение последним аргументом, иначе управление идёт в `next`.
    Catch {
        failure: ValueId,
        ty: CatchType,
        handler: Target,
        next: Target,
    },
    /// `xs[i] else fallback`: `found` получает элемент последним аргументом.
    ArrayLookup {
        array: ValueId,
        index: ValueId,
        found: Target,
        missing: Target,
    },
    /// `m[k] else fallback`: `found` получает значение последним аргументом.
    MapLookup {
        map: ValueId,
        key: ValueId,
        found: Target,
        missing: Target,
    },
}

/// Тип в `catch`: конкретный объект, делающий `Error`, или роль `Error`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CatchType {
    Object(ObjectId),
    Role(RoleId),
}

impl CatchType {
    pub fn ty(self) -> Type {
        match self {
            CatchType::Object(object) => Type::Object(object),
            CatchType::Role(role) => Type::Role(role),
        }
    }
}

impl TermKind {
    pub fn targets(&self) -> Vec<&Target> {
        match self {
            TermKind::Br(target) => vec![target],
            TermKind::Cond {
                then_target,
                else_target,
                ..
            } => vec![then_target, else_target],
            TermKind::Catch { handler, next, .. } => vec![handler, next],
            TermKind::ArrayLookup { found, missing, .. }
            | TermKind::MapLookup { found, missing, .. } => vec![found, missing],
            TermKind::Ret(_)
            | TermKind::Panic(_)
            | TermKind::Trap { .. }
            | TermKind::Unreachable
            | TermKind::Throw(_)
            | TermKind::Raise(_) => Vec::new(),
        }
    }

    pub fn targets_mut(&mut self) -> Vec<&mut Target> {
        match self {
            TermKind::Br(target) => vec![target],
            TermKind::Cond {
                then_target,
                else_target,
                ..
            } => vec![then_target, else_target],
            TermKind::Catch { handler, next, .. } => vec![handler, next],
            TermKind::ArrayLookup { found, missing, .. }
            | TermKind::MapLookup { found, missing, .. } => vec![found, missing],
            TermKind::Ret(_)
            | TermKind::Panic(_)
            | TermKind::Trap { .. }
            | TermKind::Unreachable
            | TermKind::Throw(_)
            | TermKind::Raise(_) => Vec::new(),
        }
    }

    /// Значения, которые читает терминатор, кроме аргументов переходов.
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            TermKind::Cond { condition, .. } => vec![*condition],
            TermKind::Ret(Some(value))
            | TermKind::Panic(value)
            | TermKind::Throw(value)
            | TermKind::Raise(value)
            | TermKind::Catch { failure: value, .. } => vec![*value],
            TermKind::ArrayLookup { array, index, .. } => vec![*array, *index],
            TermKind::MapLookup { map, key, .. } => vec![*map, *key],
            TermKind::Br(_)
            | TermKind::Ret(None)
            | TermKind::Trap { .. }
            | TermKind::Unreachable => Vec::new(),
        }
    }

    pub fn operands_mut(&mut self) -> Vec<&mut ValueId> {
        match self {
            TermKind::Cond { condition, .. } => vec![condition],
            TermKind::Ret(Some(value))
            | TermKind::Panic(value)
            | TermKind::Throw(value)
            | TermKind::Raise(value)
            | TermKind::Catch { failure: value, .. } => vec![value],
            TermKind::ArrayLookup { array, index, .. } => vec![array, index],
            TermKind::MapLookup { map, key, .. } => vec![map, key],
            TermKind::Br(_)
            | TermKind::Ret(None)
            | TermKind::Trap { .. }
            | TermKind::Unreachable => Vec::new(),
        }
    }

    /// Терминатор передаёт отказ (в цель unwind блока или из функции).
    pub fn fails(&self) -> bool {
        matches!(
            self,
            TermKind::Panic(_)
                | TermKind::Trap { .. }
                | TermKind::Unreachable
                | TermKind::Throw(_)
                | TermKind::Raise(_)
        )
    }
}
