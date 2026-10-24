//! Программа Sparrow — регистровый байткод Okkama (SPB 9), независимый от
//! компилятора. Генератор переводит OIR-1 в эту модель (как в JVM-байткод),
//! VM исполняет её над общим рантаймом `okkama-runtime-core`.
//!
//! Функция — плоский массив инструкций над регистрами кадра; переходы — по
//! индексам инструкций с параллельными присваиваниями регистров
//! ([`Target`]); отказы ловит таблица [`Handler`] диапазонов инструкций.
//! Таблицы типов, объектов, ролей, enum, caps и функций — часть файла: VM не
//! нужен исходник. Бинарная форма — [`encode`]/[`decode`], текстовая
//! (ассемблер) — [`assemble`]/[`disassemble`]; обе однозначны и обратимы.

mod asm;
mod codec;
mod operand;
mod verify;

pub use asm::{AsmError, assemble, disassemble};
pub use codec::{CodecError, MAX_FILE_BYTES, VERSION, decode, encode};
pub use okkama_runtime_abi as abi;
pub use operand::{Access, Operand};

use std::fmt;

/// Регистр кадра (читаемый операнд).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Register(pub u32);

/// Регистр-назначение инструкции.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dst(pub Register);

/// Индексы таблиц программы.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FuncRef(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectRef(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleRef(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnumRef(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CapRef(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeRef(pub u32);

/// Переход: индекс инструкции и параллельные присваивания (назначение,
/// источник) — сначала читаются все источники, затем пишутся назначения.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Target {
    pub pc: u32,
    pub moves: Vec<(Register, Register)>,
}

/// Константа инструкции `const`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Constant {
    /// Каноническая десятичная запись целого любой разрядности.
    Int(String),
    Bool(bool),
    Byte(u8),
    /// Биты конечного binary64.
    Float(u64),
    Char(char),
    Str(String),
}

macro_rules! op_enum {
    ($(#[$doc:meta])* $name:ident { $($variant:ident = $code:literal, $text:literal),* $(,)? }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),* }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            pub fn code(self) -> u8 {
                match self { $($name::$variant => $code),* }
            }

            pub fn from_code(code: u8) -> Option<Self> {
                match code { $($code => Some($name::$variant),)* _ => None }
            }

            pub fn name(self) -> &'static str {
                match self { $($name::$variant => $text),* }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($text => Some($name::$variant),)* _ => None }
            }
        }
    };
}

op_enum! {
    /// Сравнение (`lt` и т. п.).
    Cmp { Eq = 0, "eq", Ne = 1, "ne", Lt = 2, "lt", Le = 3, "le", Gt = 4, "gt", Ge = 5, "ge" }
}

op_enum! {
    /// Битовое действие над словом `int` или `byte` (§16.2).
    BitOp {
        BitCount = 0, "bit_count",
        LeadingZeros = 1, "leading_zeros",
        TrailingZeros = 2, "trailing_zeros",
        RotateLeft = 3, "rotl",
        RotateRight = 4, "rotr",
        ReverseBits = 5, "reverse_bits",
        SwapBytes = 6, "swap_bytes",
    }
}

impl BitOp {
    /// Результат — число битов (`int`).
    pub fn counts(self) -> bool {
        matches!(self, BitOp::BitCount | BitOp::LeadingZeros | BitOp::TrailingZeros)
    }
}

op_enum! {
    /// Наблюдение и отмена Future (§13.2).
    FutOp { Cancel = 0, "cancel", Done = 1, "done", Cancelled = 2, "cancelled" }
}

op_enum! {
    /// Вид типа в `catch`.
    CatchKind { Object = 0, "object", Role = 1, "role" }
}

op_enum! {
    /// Код программной ошибки `trap` (§5.3).
    TrapCode { Confirm = 0, "confirm", Range = 1, "range", Empty = 2, "empty", Unbound = 3, "unbound", Unreachable = 4, "unreachable" }
}

macro_rules! instructions {
    ($( $(#[$doc:meta])* $name:ident = $code:literal, $mnemonic:literal { $($field:ident : $ty:ty),* $(,)? } )*) => {
        /// Инструкция Sparrow. Порядок полей — порядок операндов в коде и ассемблере.
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum Instruction {
            $( $(#[$doc])* $name { $($field: $ty),* }, )*
        }

        impl Instruction {
            pub fn opcode(&self) -> u8 {
                match self { $( Instruction::$name { .. } => $code, )* }
            }

            pub fn mnemonic(&self) -> &'static str {
                match self { $( Instruction::$name { .. } => $mnemonic, )* }
            }

            /// Все мнемоники в порядке кодов операций.
            pub const MNEMONICS: &'static [&'static str] = &[$($mnemonic),*];

            pub(crate) fn encode_operands(&self, writer: &mut codec::Writer) {
                match self {
                    $( #[allow(unused_variables)] Instruction::$name { $($field),* } => { $( Operand::encode($field, writer); )* } )*
                }
            }

            pub(crate) fn decode_operands(code: u8, reader: &mut codec::Reader<'_>) -> Result<Self, CodecError> {
                match code {
                    $( $code => Ok(Instruction::$name { $($field: <$ty as Operand>::decode(reader)?),* }), )*
                    other => Err(reader.error(format!("unknown opcode {other}"))),
                }
            }

            pub(crate) fn print_operands(&self, out: &mut String) {
                match self {
                    $( #[allow(unused_variables, unused_mut, unused_assignments)] Instruction::$name { $($field),* } => {
                        let mut first = true;
                        $(
                            out.push_str(if first { " " } else { ", " });
                            first = false;
                            Operand::print($field, out);
                        )*
                    } )*
                }
            }

            pub(crate) fn parse_operands(mnemonic: &str, parser: &mut asm::Parser<'_>) -> Result<Self, AsmError> {
                match mnemonic {
                    $( #[allow(unused_mut, unused_variables, unused_assignments)] $mnemonic => {
                        let mut first = true;
                        $(
                            if !first { parser.comma()?; }
                            first = false;
                            let $field = <$ty as Operand>::parse(parser)?;
                        )*
                        Ok(Instruction::$name { $($field),* })
                    } )*
                    other => Err(parser.error(format!("unknown instruction `{other}`"))),
                }
            }

            /// Обход операндов: регистры, переходы и ссылки на таблицы.
            pub fn visit(&self, visitor: &mut dyn FnMut(Access<'_>)) {
                match self {
                    $( #[allow(unused_variables)] Instruction::$name { $($field),* } => { $( Operand::visit($field, visitor); )* } )*
                }
            }
        }
    };
}

instructions! {
    /// Константа.
    Const = 1, "const" { dst: Dst, value: Constant }
    /// Копия значения: `succ.take`, `retag`, `map.retag`, `cap.make`.
    Move = 2, "move" { dst: Dst, source: Register }
    /// Заглушка значения типа назначения: читать её можно только под проверкой связывания.
    Undef = 3, "undef" { dst: Dst }

    IntAdd = 10, "int.add" { dst: Dst, left: Register, right: Register }
    IntSub = 11, "int.sub" { dst: Dst, left: Register, right: Register }
    IntMul = 12, "int.mul" { dst: Dst, left: Register, right: Register }
    /// Остаток усечённого деления; делитель 0 — программная ошибка.
    IntMod = 13, "int.mod" { dst: Dst, left: Register, right: Register }
    IntAnd = 14, "int.and" { dst: Dst, left: Register, right: Register }
    IntOr = 15, "int.or" { dst: Dst, left: Register, right: Register }
    IntXor = 16, "int.xor" { dst: Dst, left: Register, right: Register }
    IntShl = 17, "int.shl" { dst: Dst, left: Register, right: Register }
    IntShr = 18, "int.shr" { dst: Dst, left: Register, right: Register }
    IntUshr = 19, "int.ushr" { dst: Dst, left: Register, right: Register }
    IntNeg = 20, "int.neg" { dst: Dst, operand: Register }
    IntNot = 21, "int.not" { dst: Dst, operand: Register }
    IntAbs = 22, "int.abs" { dst: Dst, operand: Register }
    /// Знак `int`, `float` или `rat`: −1, 0 или 1.
    Sign = 23, "sign" { dst: Dst, operand: Register }
    IntCmp = 24, "int.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }
    /// `int|float|rat → byte`: вне 0..255 — программная ошибка с префиксом `context`.
    ByteFrom = 25, "byte.from" { dst: Dst, source: Register, context: Option<String> }
    /// Преобразование §16.1; целевой тип — тип регистра назначения.
    Convert = 26, "cvt" { dst: Dst, source: Register }
    Bits = 27, "bits" { op: BitOp, byte: bool, dst: Dst, value: Register, count: Option<Register> }

    ByteAnd = 30, "byte.and" { dst: Dst, left: Register, right: Register }
    ByteOr = 31, "byte.or" { dst: Dst, left: Register, right: Register }
    ByteXor = 32, "byte.xor" { dst: Dst, left: Register, right: Register }
    ByteShl = 33, "byte.shl" { dst: Dst, left: Register, right: Register }
    ByteShr = 34, "byte.shr" { dst: Dst, left: Register, right: Register }
    ByteUshr = 35, "byte.ushr" { dst: Dst, left: Register, right: Register }
    ByteNot = 36, "byte.not" { dst: Dst, operand: Register }
    ByteCmp = 37, "byte.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }
    CharCmp = 38, "char.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }

    BoolNot = 40, "bool.not" { dst: Dst, operand: Register }
    BoolCmp = 41, "bool.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }

    FloatAdd = 50, "float.add" { dst: Dst, left: Register, right: Register }
    FloatSub = 51, "float.sub" { dst: Dst, left: Register, right: Register }
    FloatMul = 52, "float.mul" { dst: Dst, left: Register, right: Register }
    FloatDiv = 53, "float.div" { dst: Dst, left: Register, right: Register }
    FloatMod = 54, "float.mod" { dst: Dst, left: Register, right: Register }
    FloatPow = 55, "float.pow" { dst: Dst, left: Register, right: Register }
    FloatNeg = 56, "float.neg" { dst: Dst, operand: Register }
    FloatAbs = 57, "float.abs" { dst: Dst, operand: Register }
    FloatSqrt = 58, "float.sqrt" { dst: Dst, operand: Register }
    FloatCmp = 59, "float.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }

    /// Дробь из двух `int`; знаменатель 0 — программная ошибка.
    RatNew = 60, "rat.new" { dst: Dst, left: Register, right: Register }
    RatAdd = 61, "rat.add" { dst: Dst, left: Register, right: Register }
    RatSub = 62, "rat.sub" { dst: Dst, left: Register, right: Register }
    RatMul = 63, "rat.mul" { dst: Dst, left: Register, right: Register }
    RatDiv = 64, "rat.div" { dst: Dst, left: Register, right: Register }
    RatMod = 65, "rat.mod" { dst: Dst, left: Register, right: Register }
    RatNeg = 66, "rat.neg" { dst: Dst, operand: Register }
    RatAbs = 67, "rat.abs" { dst: Dst, operand: Register }
    RatCmp = 68, "rat.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }
    /// Числитель или знаменатель (`denominator`) дроби как `str`.
    RatPart = 69, "rat.part" { dst: Dst, value: Register, denominator: bool }

    StrConcat = 70, "str.concat" { dst: Dst, left: Register, right: Register }
    StrCmp = 71, "str.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }
    StrLen = 72, "str.len" { dst: Dst, text: Register }
    StrSlice = 73, "str.slice" { dst: Dst, text: Register, start: Register, end: Register }
    StrSplit = 74, "str.split" { dst: Dst, text: Register, separator: Register }
    StrReplace = 75, "str.replace" { dst: Dst, text: Register, pattern: Register, replacement: Register }
    StrReplaceFirst = 76, "str.replace_first" { dst: Dst, text: Register, pattern: Register, replacement: Register }
    StrTrim = 77, "str.trim" { dst: Dst, text: Register }
    StrChomp = 78, "str.chomp" { dst: Dst, text: Register }
    StrUtf8 = 79, "str.utf8" { dst: Dst, text: Register }

    /// Глубокое равенство (§16.3), только `eq`/`ne`.
    ValCmp = 80, "val.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }
    EnumCase = 81, "enum.case" { dst: Dst, ty: EnumRef, case: u32 }
    EnumCmp = 82, "enum.cmp" { op: Cmp, dst: Dst, left: Register, right: Register }
    AnyBox = 83, "any.box" { dst: Dst, source: Register }
    RangeNew = 84, "range.new" { dst: Dst, start: Register, end: Register, inclusive: bool }
    /// `range.start` или (`end`) эксклюзивная верхняя граница.
    RangePart = 85, "range.part" { dst: Dst, range: Register, end: bool }

    ObjNew = 90, "obj.new" { dst: Dst, object: ObjectRef, fields: Vec<Register> }
    /// Связать поле объекта, чей `self` ушёл значением до конца конструктора (§11.3).
    ObjInit = 91, "obj.init" { object: Register, field: u32, value: Register }
    ObjGet = 92, "obj.get" { dst: Dst, object: Register, field: u32 }
    /// Successor: новый объект из полей с заменами.
    ObjNext = 93, "obj.next" { dst: Dst, object: Register, updates: Vec<(u32, Register)> }
    /// Конкретный объект → ссылка роли; объект обязан её делать.
    RoleWrap = 94, "role.wrap" { dst: Dst, source: Register, role: RoleRef }
    /// Ссылка роли → ссылка другой роли; иначе программная ошибка `message`.
    RoleNarrow = 95, "role.narrow" { dst: Dst, source: Register, role: RoleRef, message: String }

    ArrayNew = 100, "arr.new" { dst: Dst, elements: Vec<Register> }
    ArrayLen = 101, "arr.len" { dst: Dst, array: Register }
    ArrayGet = 102, "arr.get" { dst: Dst, array: Register, index: Register }
    ArraySlice = 103, "arr.slice" { dst: Dst, array: Register, start: Register, end: Register }
    ArrayJoin = 104, "arr.join" { dst: Dst, array: Register, separator: Register }
    BuilderNew = 105, "builder.new" { dst: Dst }
    BuilderPush = 106, "builder.push" { dst: Dst, builder: Register, value: Register }
    BuilderAppend = 107, "builder.append" { dst: Dst, builder: Register, array: Register }
    BuilderRange = 108, "builder.range" { dst: Dst, builder: Register, start: Register, end: Register, inclusive: bool }
    BuilderFinish = 109, "builder.finish" { dst: Dst, builder: Register }

    MapNew = 110, "map.new" { dst: Dst, entries: Vec<(Register, Register)> }
    MapLen = 111, "map.len" { dst: Dst, map: Register }
    MapHas = 112, "map.has" { dst: Dst, map: Register, key: Register }
    MapGet = 113, "map.get" { dst: Dst, map: Register, key: Register }
    MapKeys = 114, "map.keys" { dst: Dst, map: Register }
    MapValues = 115, "map.values" { dst: Dst, map: Register }
    MapBuilderNew = 116, "mbuilder.new" { dst: Dst }
    MapBuilderPut = 117, "mbuilder.put" { dst: Dst, builder: Register, key: Register, value: Register }
    MapBuilderFinish = 118, "mbuilder.finish" { dst: Dst, builder: Register }

    /// Прямой вызов; результат async-функции — холодный Future.
    Call = 120, "call" { dst: Option<Dst>, function: FuncRef, args: Vec<Register> }
    /// Значение-функция над захватами.
    Closure = 121, "fn.closure" { dst: Dst, function: FuncRef, captures: Vec<Register> }
    CallFn = 122, "call.fn" { dst: Option<Dst>, callee: Register, args: Vec<Register> }
    /// Слот роли на ссылке роли `role`.
    CallRole = 123, "call.role" { dst: Option<Dst>, receiver: Register, role: RoleRef, slot: u32, args: Vec<Register> }
    /// Поле cap вычисляется заново над владельцем (§8.5).
    CallCap = 124, "call.cap" { dst: Dst, cap: Register, ty: CapRef, field: u32 }
    Await = 125, "await" { dst: Option<Dst>, future: Register }
    /// Запустить Future сейчас; результат — тот же Future.
    FutSpawn = 126, "fut.spawn" { dst: Dst, future: Register }
    Fut = 127, "fut" { op: FutOp, dst: Option<Dst>, future: Register }
    /// Синхронный action в процессе-воркере (§18.1).
    WorkerSpawn = 128, "worker.spawn" { dst: Dst, entry: Register, args: Vec<Register>, direct: bool }
    WorkerBind = 129, "worker.bind" { dst: Dst, action: Register, entry: Register, fields: Vec<Register> }

    /// Регион владения области (§7).
    RegionEnter = 130, "region.enter" { dst: Dst }
    RegionAdopt = 131, "region.adopt" { region: Register, value: Register }
    RegionTransfer = 132, "region.transfer" { from: Register, to: Register, value: Register }
    RegionTrack = 133, "region.track" { region: Register, value: Register }
    /// Закрыть регион: `destroy` его идентичностей (кроме достижимых из `keep`).
    RegionLeave = 134, "region.leave" { region: Register, keep: Option<Register> }
    /// Закрыть регион на пути отказа; результат — отказ с вторичными отказами destroy.
    RegionUnwind = 135, "region.unwind" { dst: Dst, region: Register, failure: Register }
    FailSecondary = 136, "fail.secondary" { dst: Dst, primary: Register, cleanup: Register }
    FailCaused = 137, "fail.caused" { dst: Dst, new: Register, handled: Register }

    Jump = 140, "jmp" { target: Target }
    Branch = 141, "br" { condition: Register, then_target: Target, else_target: Target }
    Return = 142, "ret" { value: Option<Register> }
    Panic = 143, "panic" { message: Register }
    Trap = 144, "trap" { code: TrapCode, message: String }
    Unreachable = 145, "unreachable" {}
    Throw = 146, "throw" { value: Register }
    Raise = 147, "raise" { failure: Register }
    /// Выбор catch (§6.4): при совпадении пойманное значение пишется в `value`
    /// после присваиваний `handler`, иначе переход в `next`.
    Catch = 148, "catch" { failure: Register, kind: CatchKind, ty: u32, value: Dst, handler: Target, next: Target }
    /// `xs[i] else …`: найденный элемент пишется в `value` на ребре `found`.
    ArrayLookup = 149, "arr.lookup" { array: Register, index: Register, value: Dst, found: Target, missing: Target }
    MapLookup = 150, "map.lookup" { map: Register, key: Register, value: Dst, found: Target, missing: Target }
}

impl Instruction {
    /// Последняя инструкция участка: управление не переходит к следующей.
    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Instruction::Jump { .. }
                | Instruction::Branch { .. }
                | Instruction::Return { .. }
                | Instruction::Panic { .. }
                | Instruction::Trap { .. }
                | Instruction::Unreachable {}
                | Instruction::Throw { .. }
                | Instruction::Raise { .. }
                | Instruction::Catch { .. }
                | Instruction::ArrayLookup { .. }
                | Instruction::MapLookup { .. }
        )
    }
}

/// Тип таблицы типов (OIR-1 §3); вложенные типы — ссылки на более ранние записи.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Bool,
    Byte,
    Float,
    Rat,
    Char,
    Str,
    Object(ObjectRef),
    Successor(ObjectRef),
    Role(RoleRef),
    Enum(EnumRef),
    Failure,
    Array(TypeRef),
    Never,
    Builder(TypeRef),
    Map(TypeRef, TypeRef),
    MapBuilder(TypeRef, TypeRef),
    Region,
    Handle(u32),
    Any,
    Range,
    Fn { params: Vec<TypeRef>, result: Option<TypeRef> },
    /// `future<R>`; `None` — `future<void>`.
    Future(Option<TypeRef>),
    Cap(CapRef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: String,
    pub cases: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleMethod {
    pub name: String,
    pub params: Vec<TypeRef>,
    pub result: Option<TypeRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleDecl {
    pub name: String,
    /// Системная роль (`Error`).
    pub builtin: bool,
    pub methods: Vec<RoleMethod>,
}

impl RoleDecl {
    pub fn is_error(&self) -> bool {
        self.builtin && self.name == "Error"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness {
    pub role: RoleRef,
    pub slots: Vec<FuncRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectDecl {
    pub name: String,
    pub fields: Vec<(String, TypeRef)>,
    pub does: Vec<Witness>,
    pub destroy: Option<FuncRef>,
    /// Системный объект рантайма (`NativeFailure`, `Cancelled`).
    pub builtin: bool,
}

impl ObjectDecl {
    pub fn is_native_failure(&self) -> bool {
        self.builtin && self.name == "NativeFailure"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapDecl {
    pub name: String,
    pub owner: ObjectRef,
    pub fields: Vec<(String, FuncRef)>,
}

/// Intrinsic (§14.2): тела нет, его даёт рантайм.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intrinsic {
    pub name: String,
    pub objects: Vec<ObjectRef>,
    pub helpers: Vec<FuncRef>,
}

/// Диапазон инструкций `[start, end)`, чей отказ переходит в `target`;
/// отказ пишется в `failure` после присваиваний перехода.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub start: u32,
    pub end: u32,
    pub target: Target,
    pub failure: Register,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub symbol: String,
    /// `async`: вызов даёт холодный Future, тело исполняется при `await`.
    pub is_async: bool,
    /// `native`: символ provider рантайма; кода нет.
    pub native: Option<String>,
    pub intrinsic: Option<Intrinsic>,
    /// Тип каждого регистра кадра.
    pub registers: Vec<TypeRef>,
    pub params: Vec<Register>,
    /// `None` — void.
    pub result: Option<TypeRef>,
    pub code: Vec<Instruction>,
    /// Упорядочены по `start`, не пересекаются.
    pub handlers: Vec<Handler>,
}

impl Function {
    /// Обработчик отказа инструкции `pc`.
    pub fn handler_at(&self, pc: u32) -> Option<&Handler> {
        let index = self.handlers.partition_point(|handler| handler.start <= pc);
        index
            .checked_sub(1)
            .and_then(|index| self.handlers.get(index))
            .filter(|handler| pc < handler.end)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Program {
    pub module: String,
    pub version: String,
    pub types: Vec<Type>,
    pub enums: Vec<EnumDecl>,
    pub roles: Vec<RoleDecl>,
    pub objects: Vec<ObjectDecl>,
    pub handles: Vec<String>,
    pub caps: Vec<CapDecl>,
    pub functions: Vec<Function>,
    pub entry: Option<FuncRef>,
    /// Тестовый профиль: имя и функция каждого теста корня.
    pub tests: Option<Vec<(String, FuncRef)>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyError {
    pub function: Option<String>,
    pub instruction: Option<usize>,
    pub message: String,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.function, self.instruction) {
            (Some(function), Some(index)) => write!(f, "Sparrow @{function} instruction {index}: {}", self.message),
            (Some(function), None) => write!(f, "Sparrow @{function}: {}", self.message),
            _ => write!(f, "Sparrow program: {}", self.message),
        }
    }
}

impl std::error::Error for VerifyError {}

impl Program {
    /// Проверяет ссылки таблиц, регистры, переходы, обработчики и
    /// инициализацию регистров на всех путях до чтения.
    pub fn verify(&self) -> Result<(), VerifyError> {
        verify::verify(self)
    }

    pub fn ty(&self, ty: TypeRef) -> Option<&Type> {
        self.types.get(ty.0 as usize)
    }

    pub fn function(&self, function: FuncRef) -> Option<&Function> {
        self.functions.get(function.0 as usize)
    }
}
