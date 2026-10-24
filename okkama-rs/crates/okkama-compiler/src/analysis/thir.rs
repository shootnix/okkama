//! THIR — типизированное дерево тел после семантического анализа.
//!
//! Все имена разрешены (локальные — [`LocalId`], поля — индексы, вызовы —
//! конкретные action, конструктор или builtin), у каждого выражения есть тип,
//! операторы выбраны по типам операндов, а неявные границы (погашение `^C`,
//! обёртка роли, тег пустой коллекции) записаны явными узлами [`Coercion`].
//! Lowering переводит THIR в OIR механически: правил языка он не проверяет.

use super::ty::{EnumId, ObjectId, RoleId, Ty};
use crate::source::Span;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);

#[derive(Clone, Debug)]
pub struct Local {
    pub name: String,
    pub ty: Ty,
    /// Rebind разрешён (тип с successor-действием, §9.2a).
    pub rebindable: bool,
    /// Variadic-привязка: читается только распаковкой `name...`.
    pub variadic: bool,
    pub span: Span,
}

/// Чьё тело.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BodyOwner {
    Entry,
    Method {
        object: ObjectId,
        method: usize,
    },
    Ctor {
        object: ObjectId,
        ctor: usize,
    },
    Default {
        object: ObjectId,
        field: usize,
    },
    Destroy {
        object: ObjectId,
    },
    CapField {
        object: ObjectId,
        cap: usize,
        field: usize,
    },
    /// Анонимное действие (тело внутри [`ExprKind::Closure`]).
    Closure,
    /// `test "name"` корневого модуля (`Registry::tests[index]`).
    Test(usize),
}

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: BodyOwner,
    /// Модуль, в чьей области разрешались имена тела.
    pub module: usize,
    pub locals: Vec<Local>,
    pub params: Vec<LocalId>,
    pub block: Block,
    /// Значение последнего оператора — результат тела (не successor, не
    /// конструктор и не `void`).
    pub returns_value: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Let {
        local: LocalId,
        value: Expr,
    },
    Expr(Expr),
    /// `stmt if condition` (постфиксное условие, `next if`, `last if`).
    Guarded {
        condition: Expr,
        stmt: Box<Stmt>,
    },
    Return(Option<Expr>),
    Confirm {
        condition: Expr,
        fallback: Option<Fallback>,
    },
    Throw(Expr),
    /// Bare `throw;` в теле `catch`.
    Rethrow,
    /// Связывание поля строящегося объекта (`field = v`, `self.field = v`).
    BindField {
        field: usize,
        value: Expr,
    },
    /// `^field = v` в successor-действии.
    StageField {
        field: usize,
        value: Expr,
    },
    Rebind {
        local: LocalId,
        value: Expr,
    },
    Defer(Expr),
    Next,
    Last(Option<Expr>),
}

#[derive(Clone, Debug)]
pub enum Fallback {
    Throw(Expr),
    Panic(Expr),
}

/// Источник обхода `for`/функциональных операций.
#[derive(Clone, Debug)]
pub enum Source {
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    Array(Box<Expr>),
    Map(Box<Expr>),
    /// Variadic-привязка, обходимая без материализации.
    Varargs(LocalId),
    /// Значение `collections/iteration.Iterating<T>` (после границы роли).
    Iterating(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    /// `None` — выражение значения не даёт (void-вызов, `if` без `else`).
    pub ty: Option<Ty>,
    /// Управление не возвращается (`return`, `panic`, бесконечный `loop`).
    pub diverges: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Constant {
    /// Каноническая десятичная запись.
    Int(String),
    /// Биты binary64.
    Float(u64),
    Str(String),
    Char(char),
    Bool(bool),
    Byte(u8),
}

/// Неявная граница типа (§12.1, §8.4).
#[derive(Clone, Debug, PartialEq)]
pub enum Coercion {
    /// `^C` → `C`: погашение обязательства successor.
    Take,
    /// Объект или builtin → роль, которую он делает.
    RoleWrap,
    /// Роль → роль: подмножество действий, проверка во время исполнения.
    RoleNarrow { message: String },
    /// `[]`/`[:]` получает тип границы.
    Retag,
    /// Значение → `any`.
    Box,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NumericKind {
    Int,
    Byte,
    Float,
    Rat,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BitOp {
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CompareOp {
    pub fn is_equality(self) -> bool {
        matches!(self, CompareOp::Eq | CompareOp::Ne)
    }
}

/// Категория сравнения операндов одного типа.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompareKind {
    Int,
    Byte,
    Float,
    Rat,
    Bool,
    Char,
    Str,
    Enum,
    /// Глубокое типизированное равенство (`val.eq`): коллекции, объекты, параметры типа.
    Value,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    /// Унарный минус; у `byte` результат — `int`.
    Negate(NumericKind),
    /// `~` над `int` или `byte`.
    BitNot {
        byte: bool,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    /// Арифметика; `Byte` — оба операнда `byte`, результат `int` (`/` — `rat`).
    Arith(NumericKind, ArithOp),
    /// Битовые операции над словом `int` (signed-64) или `byte`.
    Bits {
        byte: bool,
        op: BitOp,
    },
    Concat,
    Compare(CompareKind, CompareOp),
}

/// Явное приведение `T(x)`, `x.T()` и элементы `@T(...)` (§6.6.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CastTarget {
    Int,
    Byte,
    Float,
    Rat,
    Str,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Constant(Constant),
    Local(LocalId),
    SelfValue,
    /// Поле receiver в action/destroy (исходное значение, D04).
    Field(usize),
    /// Текущее значение поля строящегося объекта или staged-поля successor.
    PendingField(usize),
    EnumCase {
        ty: EnumId,
        case: usize,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `&&` (`and = true`) и `||` с коротким замыканием.
    Logical {
        and: bool,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Coerce {
        coercion: Coercion,
        value: Box<Expr>,
    },
    /// Явное приведение; `position` — номер операнда `@T(...)`.
    Cast {
        target: CastTarget,
        value: Box<Expr>,
        position: Option<usize>,
    },
    /// `rat(n, d)`.
    RatNew {
        numerator: Box<Expr>,
        denominator: Box<Expr>,
    },
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_block: Option<Block>,
    },
    Match(Box<Match>),
    Loop(Block),
    /// `for source -> binding { body }`; в позиции значения — значение
    /// последней итерации или `last value` (§9.10).
    For {
        source: Source,
        binding: LocalId,
        body: Block,
    },
    Try(Box<Try>),
    Block(Block),
    Panic(Box<Expr>),
    Call {
        callee: Callee,
        args: Vec<Expr>,
    },
    Array(Vec<ArrayPart>),
    Map(Vec<(Expr, Expr)>),
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        fallback: Option<Box<Expr>>,
    },
    Functional(Box<Functional>),
    ArrayCast {
        element: CastTarget,
        parts: Vec<CastPart>,
    },
    /// `prefix Object(...)`: конструирование, затем действие `#projected`.
    Projection {
        construction: Box<Expr>,
        object: ObjectId,
        method: usize,
    },
    /// Новый массив из распаковки `xs...` (массив или variadic-привязка).
    Unpack(Box<Expr>),
    /// Variadic-хвост вызова: отдельные значения и необязательная распаковка.
    Pack {
        items: Vec<Expr>,
        spread: Option<Box<Expr>>,
    },
    Await(Box<Expr>),
    Spawn(Box<Expr>),
    /// Анонимное действие: тело и захваченные локальные внешнего тела.
    Closure {
        body: Box<Body>,
        captures: Vec<(LocalId, LocalId)>,
    },
    /// Контекстная возможность `test`.
    TestCapability,
    /// Bound action ограниченной ссылки роли.
    BoundRoleAction {
        receiver: Box<Expr>,
        role: RoleId,
        slot: usize,
    },
    /// `receiver.action` без вызова (§8.10.1).
    BoundAction {
        receiver: Box<Expr>,
        object: ObjectId,
        method: usize,
    },
    /// Вызов action-значения.
    CallValue {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// `sink <- source`: значение — исходный sink.
    StreamWrite {
        sink: Box<Expr>,
        source: Box<Expr>,
        unpack: bool,
    },
    /// `owner::Cap`.
    CapSelect {
        owner: Box<Expr>,
        object: ObjectId,
        cap: usize,
    },
    /// `cap::field`.
    CapRead {
        cap: Box<Expr>,
        object: ObjectId,
        index: usize,
        field: usize,
    },
    /// `Owner::field` в теле внешнего cap.
    OwnerField(usize),
    /// Код после известного ухода управления (проверен, не исполняется).
    Unreachable,
}

#[derive(Clone, Debug)]
pub enum ArrayPart {
    Value(Expr),
    Range {
        start: Expr,
        end: Expr,
        inclusive: bool,
    },
}

#[derive(Clone, Debug)]
pub enum CastPart {
    One(Expr),
    Spread(Expr),
}

#[derive(Clone, Debug)]
pub enum Callee {
    Method {
        receiver: Box<Expr>,
        object: ObjectId,
        method: usize,
    },
    Role {
        receiver: Box<Expr>,
        role: RoleId,
        slot: usize,
    },
    /// Конструктор; `given` — переданные поля shorthand (по возрастанию индекса).
    Ctor {
        object: ObjectId,
        type_args: Vec<Ty>,
        ctor: usize,
        given: Vec<usize>,
    },
    Native(usize),
    Builtin {
        receiver: Box<Expr>,
        op: Builtin,
    },
    /// `test.ok(...)`, `test.is(...)`, `test.isnt(...)`, `test.fail(...)`.
    Test(String),
    /// `fake.storage.set/get/has/remove/clear/size(...)` в action fake (§14):
    /// хранилище экземпляра `self`.
    FakeStorage(String),
}

/// Builtin-действия значений (§6.6).
#[derive(Clone, Debug, PartialEq)]
pub enum Builtin {
    ArraySize,
    ArraySlice,
    ArrayJoin,
    ArrayItem,
    ArrayFirst,
    ArrayLast,
    ArrayCursor,
    /// `@T.ordering()` по `compare.Comparing<T>` элементов.
    ArrayOrdering,
    StrLength,
    StrEmpty,
    StrSlice,
    StrSplit,
    StrReplace,
    StrReplaceFirst,
    StrTrim,
    StrChomp,
    /// Строки упорядочиваются по длине (как Perl): `less`, `less_or_equal`.
    StrCompare(CompareOp),
    MapSize,
    MapHas,
    MapKeys,
    MapValues,
    /// `less`/`more`/... одной категории.
    Compare(CompareKind, CompareOp),
    /// `order(other)` → `compare.Order`.
    Order(CompareKind),
    Abs,
    Sign,
    Sqrt,
    Pow,
    /// `bit_count`, `rotate_left`, ... (`byte` — над словом 8 бит).
    Bits {
        name: String,
        byte: bool,
    },
    /// `x.int()`, `x.string()`, ... без аргументов.
    Convert(CastTarget),
    Numerator,
    Denominator,
    FutureCancel,
    FutureDone,
    FutureCancelled,
}

#[derive(Clone, Debug)]
pub struct Match {
    pub subject: Expr,
    pub cases: Vec<(Vec<Expr>, Block)>,
    pub else_block: Option<Block>,
    /// Образцы-константы покрывают все значения: последний case без проверки.
    pub exhaustive: bool,
}

#[derive(Clone, Debug)]
pub enum CatchType {
    Object(ObjectId, Vec<Ty>),
    Role(RoleId, Vec<Ty>),
}

#[derive(Clone, Debug)]
pub struct Catch {
    pub local: LocalId,
    pub ty: CatchType,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Try {
    pub body: Block,
    pub catches: Vec<Catch>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FunctionalOp {
    Map,
    Filter,
    Reduce,
}

#[derive(Clone, Debug)]
pub struct Functional {
    pub op: FunctionalOp,
    pub source: Source,
    pub initial: Option<Expr>,
    /// Аккумулятор (reduce), затем элемент (или ключ и значение словаря).
    pub bindings: Vec<LocalId>,
    pub body: Block,
}
