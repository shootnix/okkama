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
    /// `a + b` над массивами одного типа: склейка (цепочка — одна инструкция).
    ArrayConcat,
    /// `a - b` над массивами: элементы `a`, не равные ни одному элементу `b`.
    ArrayDifference,
    /// `a + b` над словарями одного типа; при повторе ключа побеждает правый.
    MapMerge,
    /// `a - b` над словарями: записи `a`, ключей которых нет в `b`.
    MapDifference,
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
    VariantNew {
        case: usize,
        values: Vec<Expr>,
    },
    VariantTag(Box<Expr>),
    VariantGet {
        value: Box<Expr>,
        field: usize,
    },
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
    /// Error enum (`enum Name is Error`): ловятся все его варианты.
    Variant(EnumId),
}

#[derive(Clone, Debug)]
pub struct Catch {
    pub local: LocalId,
    /// Тип привязки: тип единственной альтернативы, иначе роль `Error`.
    pub ty: CatchType,
    /// Альтернативы в порядке исходника; первая совпавшая выбирает этот catch.
    pub alternatives: Vec<CatchAlt>,
    pub body: Block,
    pub span: Span,
}

/// Альтернатива catch: тип и, для error enum, допустимые варианты.
#[derive(Clone, Debug)]
pub struct CatchAlt {
    pub ty: CatchType,
    pub cases: Option<Vec<usize>>,
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

/// Does `body` read the receiver of the action it is written in — `self`, a
/// field, `fake.storage`, or a nested anonymous action that does? An
/// anonymous action captures `self` only then (#53): unreferenced values are
/// not retained. The walk names every form, so a new one must decide here.
pub fn uses_receiver(body: &Body) -> bool {
    block_uses_receiver(&body.block)
}

fn block_uses_receiver(block: &Block) -> bool {
    block.stmts.iter().any(stmt_uses_receiver)
}

fn stmt_uses_receiver(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::Rebind { value, .. } => expr_uses_receiver(value),
        StmtKind::Expr(value) | StmtKind::Throw(value) | StmtKind::Defer(value) => expr_uses_receiver(value),
        StmtKind::Guarded { condition, stmt } => expr_uses_receiver(condition) || stmt_uses_receiver(stmt),
        StmtKind::Return(value) | StmtKind::Last(value) => value.as_ref().is_some_and(expr_uses_receiver),
        StmtKind::Confirm { condition, fallback } => {
            expr_uses_receiver(condition)
                || fallback.as_ref().is_some_and(|fallback| match fallback {
                    Fallback::Throw(value) | Fallback::Panic(value) => expr_uses_receiver(value),
                })
        }
        // Fields of the object being built or succeeded belong to the receiver.
        StmtKind::BindField { .. } | StmtKind::StageField { .. } => true,
        StmtKind::Rethrow | StmtKind::Next => false,
    }
}

fn source_uses_receiver(source: &Source) -> bool {
    match source {
        Source::Range { start, end, .. } => expr_uses_receiver(start) || expr_uses_receiver(end),
        Source::Array(value) | Source::Map(value) | Source::Iterating(value) => expr_uses_receiver(value),
        Source::Varargs(_) => false,
    }
}

fn expr_uses_receiver(expr: &Expr) -> bool {
    let any = |values: &[Expr]| values.iter().any(expr_uses_receiver);
    match &expr.kind {
        ExprKind::SelfValue | ExprKind::Field(_) | ExprKind::PendingField(_) | ExprKind::OwnerField(_) => true,
        ExprKind::Constant(_)
        | ExprKind::Local(_)
        | ExprKind::EnumCase { .. }
        | ExprKind::TestCapability
        | ExprKind::Unreachable => false,
        ExprKind::VariantNew { values, .. } => any(values),
        ExprKind::VariantTag(value)
        | ExprKind::VariantGet { value, .. }
        | ExprKind::Unary { operand: value, .. }
        | ExprKind::Coerce { value, .. }
        | ExprKind::Cast { value, .. }
        | ExprKind::Panic(value)
        | ExprKind::Unpack(value)
        | ExprKind::Await(value)
        | ExprKind::Spawn(value) => expr_uses_receiver(value),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Logical { left, right, .. }
        | ExprKind::RatNew { numerator: left, denominator: right }
        | ExprKind::Range { start: left, end: right, .. }
        | ExprKind::StreamWrite { sink: left, source: right, .. } => expr_uses_receiver(left) || expr_uses_receiver(right),
        ExprKind::If { condition, then_block, else_block } => {
            expr_uses_receiver(condition)
                || block_uses_receiver(then_block)
                || else_block.as_ref().is_some_and(block_uses_receiver)
        }
        ExprKind::Match(found) => {
            expr_uses_receiver(&found.subject)
                || found.cases.iter().any(|(patterns, block)| any(patterns) || block_uses_receiver(block))
                || found.else_block.as_ref().is_some_and(block_uses_receiver)
        }
        ExprKind::Loop(block) | ExprKind::Block(block) => block_uses_receiver(block),
        ExprKind::For { source, body, .. } => source_uses_receiver(source) || block_uses_receiver(body),
        ExprKind::Try(found) => {
            block_uses_receiver(&found.body) || found.catches.iter().any(|catch| block_uses_receiver(&catch.body))
        }
        ExprKind::Call { callee, args } => {
            any(args)
                || match callee {
                    Callee::Method { receiver, .. } | Callee::Role { receiver, .. } | Callee::Builtin { receiver, .. } => {
                        expr_uses_receiver(receiver)
                    }
                    // The storage of the receiver's fake.
                    Callee::FakeStorage(_) => true,
                    Callee::Ctor { .. } | Callee::Native(_) | Callee::Test(_) => false,
                }
        }
        ExprKind::Array(parts) => parts.iter().any(|part| match part {
            ArrayPart::Value(value) => expr_uses_receiver(value),
            ArrayPart::Range { start, end, .. } => expr_uses_receiver(start) || expr_uses_receiver(end),
        }),
        ExprKind::Map(entries) => entries.iter().any(|(key, value)| expr_uses_receiver(key) || expr_uses_receiver(value)),
        ExprKind::Index { object, index, fallback } => {
            expr_uses_receiver(object) || expr_uses_receiver(index) || fallback.as_deref().is_some_and(expr_uses_receiver)
        }
        ExprKind::Functional(found) => {
            source_uses_receiver(&found.source)
                || found.initial.as_ref().is_some_and(expr_uses_receiver)
                || block_uses_receiver(&found.body)
        }
        ExprKind::ArrayCast { parts, .. } => parts.iter().any(|part| match part {
            CastPart::One(value) | CastPart::Spread(value) => expr_uses_receiver(value),
        }),
        ExprKind::Pack { items, spread } => any(items) || spread.as_deref().is_some_and(expr_uses_receiver),
        // A nested anonymous action takes `self` from this one.
        ExprKind::Closure { body, .. } => uses_receiver(body),
        ExprKind::BoundRoleAction { receiver, .. } | ExprKind::BoundAction { receiver, .. } => expr_uses_receiver(receiver),
        ExprKind::CallValue { callee, args } => expr_uses_receiver(callee) || any(args),
        ExprKind::CapSelect { owner, .. } => expr_uses_receiver(owner),
        ExprKind::CapRead { cap, .. } => expr_uses_receiver(cap),
    }
}

/// How many times `body` names `local`: reads, rebinds and captures by an
/// anonymous action. Lowering keeps a `reduce` accumulator in a builder only
/// when every mention is one it can rewrite (#243). The walk names every
/// form, so a new one must decide here.
pub fn local_mentions(block: &Block, local: LocalId) -> usize {
    block.stmts.iter().map(|stmt| stmt_mentions(stmt, local)).sum()
}

fn stmt_mentions(stmt: &Stmt, local: LocalId) -> usize {
    let one = |value: &Expr| expr_mentions(value, local);
    match &stmt.kind {
        StmtKind::Let { value, .. }
        | StmtKind::Expr(value)
        | StmtKind::Throw(value)
        | StmtKind::Defer(value)
        | StmtKind::BindField { value, .. }
        | StmtKind::StageField { value, .. } => one(value),
        StmtKind::Rebind { local: target, value } => usize::from(*target == local) + one(value),
        StmtKind::Guarded { condition, stmt } => one(condition) + stmt_mentions(stmt, local),
        StmtKind::Return(value) | StmtKind::Last(value) => value.as_ref().map_or(0, one),
        StmtKind::Confirm { condition, fallback } => {
            one(condition)
                + fallback.as_ref().map_or(0, |fallback| match fallback {
                    Fallback::Throw(value) | Fallback::Panic(value) => one(value),
                })
        }
        StmtKind::Rethrow | StmtKind::Next => 0,
    }
}

fn source_mentions(source: &Source, local: LocalId) -> usize {
    match source {
        Source::Range { start, end, .. } => expr_mentions(start, local) + expr_mentions(end, local),
        Source::Array(value) | Source::Map(value) | Source::Iterating(value) => expr_mentions(value, local),
        Source::Varargs(varargs) => usize::from(*varargs == local),
    }
}

pub fn expr_mentions(expr: &Expr, local: LocalId) -> usize {
    let one = |value: &Expr| expr_mentions(value, local);
    let all = |values: &[Expr]| values.iter().map(|value| expr_mentions(value, local)).sum::<usize>();
    let block = |block: &Block| local_mentions(block, local);
    match &expr.kind {
        ExprKind::Local(found) => usize::from(*found == local),
        ExprKind::Constant(_)
        | ExprKind::SelfValue
        | ExprKind::Field(_)
        | ExprKind::PendingField(_)
        | ExprKind::OwnerField(_)
        | ExprKind::EnumCase { .. }
        | ExprKind::TestCapability
        | ExprKind::Unreachable => 0,
        ExprKind::VariantNew { values, .. } => all(values),
        ExprKind::VariantTag(value)
        | ExprKind::VariantGet { value, .. }
        | ExprKind::Unary { operand: value, .. }
        | ExprKind::Coerce { value, .. }
        | ExprKind::Cast { value, .. }
        | ExprKind::Panic(value)
        | ExprKind::Unpack(value)
        | ExprKind::Await(value)
        | ExprKind::Spawn(value) => one(value),
        ExprKind::Binary { left, right, .. }
        | ExprKind::Logical { left, right, .. }
        | ExprKind::RatNew { numerator: left, denominator: right }
        | ExprKind::Range { start: left, end: right, .. }
        | ExprKind::StreamWrite { sink: left, source: right, .. } => one(left) + one(right),
        ExprKind::If { condition, then_block, else_block } => {
            one(condition) + block(then_block) + else_block.as_ref().map_or(0, block)
        }
        ExprKind::Match(found) => {
            one(&found.subject)
                + found.cases.iter().map(|(patterns, body)| all(patterns) + block(body)).sum::<usize>()
                + found.else_block.as_ref().map_or(0, block)
        }
        ExprKind::Loop(body) | ExprKind::Block(body) => block(body),
        ExprKind::For { source, body, .. } => source_mentions(source, local) + block(body),
        ExprKind::Try(found) => block(&found.body) + found.catches.iter().map(|catch| block(&catch.body)).sum::<usize>(),
        ExprKind::Call { callee, args } => {
            all(args)
                + match callee {
                    Callee::Method { receiver, .. } | Callee::Role { receiver, .. } | Callee::Builtin { receiver, .. } => {
                        one(receiver)
                    }
                    Callee::FakeStorage(_) | Callee::Ctor { .. } | Callee::Native(_) | Callee::Test(_) => 0,
                }
        }
        ExprKind::Array(parts) => parts
            .iter()
            .map(|part| match part {
                ArrayPart::Value(value) => one(value),
                ArrayPart::Range { start, end, .. } => one(start) + one(end),
            })
            .sum(),
        ExprKind::Map(entries) => entries.iter().map(|(key, value)| one(key) + one(value)).sum(),
        ExprKind::Index { object, index, fallback } => one(object) + one(index) + fallback.as_deref().map_or(0, one),
        ExprKind::Functional(found) => {
            source_mentions(&found.source, local) + found.initial.as_ref().map_or(0, one) + block(&found.body)
        }
        ExprKind::ArrayCast { parts, .. } => parts
            .iter()
            .map(|part| match part {
                CastPart::One(value) | CastPart::Spread(value) => one(value),
            })
            .sum(),
        ExprKind::Pack { items, spread } => all(items) + spread.as_deref().map_or(0, one),
        // An anonymous action names an outer local only through a capture.
        ExprKind::Closure { captures, .. } => captures.iter().filter(|(outer, _)| *outer == local).count(),
        ExprKind::BoundRoleAction { receiver, .. } | ExprKind::BoundAction { receiver, .. } => one(receiver),
        ExprKind::CallValue { callee, args } => one(callee) + all(args),
        ExprKind::CapSelect { owner, .. } => one(owner),
        ExprKind::CapRead { cap, .. } => one(cap),
    }
}
