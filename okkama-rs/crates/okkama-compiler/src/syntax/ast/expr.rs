//! Expressions.

use super::{ActionResult, Block, MatchBody, Name, Param, TryBody};
use crate::source::Span;
use crate::syntax::token::{FloatLiteral, IntLiteral, StrLiteral};

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    /// A variable or any other bare name (types, actions, `test`, ...).
    Name(Name),
    /// `self`
    SelfValue,
    /// `( expr )` — kept because grouping changes how a reader (and the
    /// `for` source rule) sees the expression.
    Grouping(Box<Expr>),
    Unary {
        op: UnaryOp,
        op_span: Span,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        op_span: Span,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `sink <- source`
    StreamWrite {
        op_span: Span,
        sink: Box<Expr>,
        source: Box<Expr>,
    },
    /// `start..end` or `start..=end`
    Range {
        inclusive: bool,
        op_span: Span,
        start: Box<Expr>,
        end: Box<Expr>,
    },
    /// `callee(args)` — calls, constructions and primitive casts such as
    /// `byte(112)` all share this node; telling them apart is semantic.
    Call {
        callee: Box<Expr>,
        args: Vec<Argument>,
    },
    /// `@int(args)` / `@byte(...)` — the array cast syntax. The target is
    /// one of `int`, `float`, `byte`, `string`.
    ArrayCast {
        marker: Span,
        target: Name,
        args: Vec<Argument>,
    },
    /// `object.name`
    Get {
        object: Box<Expr>,
        name: Name,
    },
    /// `object::name` — cap selection.
    Select {
        object: Box<Expr>,
        name: Name,
    },
    /// `Owner::field` inside an external cap body whose owner is `Owner`.
    OwnerField {
        owner: Name,
        name: Name,
    },
    /// `object[index]` with an optional `else fallback`.
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        fallback: Option<Box<Expr>>,
    },
    /// Postfix unpack `operand...`.
    Unpack {
        operand: Box<Expr>,
        ellipsis: Span,
    },
    /// Named constructor projection `prefix Type(args)`; `construction` is
    /// always the `Call` of the constructor.
    Projection {
        prefix: Name,
        construction: Box<Expr>,
    },
    /// `receiver.map() -> a { }`, `.filter() -> a { }`, `.reduce(init) -> acc, a { }`.
    Functional {
        operation: FunctionalOp,
        operation_name: Name,
        receiver: Box<Expr>,
        args: Vec<Argument>,
        bindings: Vec<Name>,
        body: Block,
    },
    /// `(a is int, b is int) is int { }`
    AnonymousAction {
        params: Vec<Param>,
        result: ActionResult,
        body: Block,
    },
    /// `[a, b]` (`[]` is empty)
    Array {
        elements: Vec<Expr>,
    },
    /// `[k: v]` (`[:]` is empty)
    Map {
        entries: Vec<MapEntry>,
    },
    /// `if cond { } else { }` in expression position (`else` is mandatory and
    /// must be a block).
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_block: Block,
    },
    /// `match` in expression position.
    Match(Box<MatchBody>),
    /// `loop { }` — produces the value of `last value`.
    Loop {
        body: Block,
    },
    /// `try { } catch ...` in expression position.
    Try(Box<TryBody>),
    /// `panic value`
    Panic {
        value: Box<Expr>,
    },
    /// `await value`
    Await {
        value: Box<Expr>,
    },
    /// `spawn call`
    Spawn {
        call: Box<Expr>,
    },
}

/// A literal value with its decoded representation.
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Int(IntLiteral),
    Float(FloatLiteral),
    String(StrLiteral),
    Char(char),
    Bool(bool),
}

/// A call argument: positional `expr` or named `name: expr`.
#[derive(Clone, Debug, PartialEq)]
pub struct Argument {
    pub name: Option<Name>,
    pub value: Expr,
    pub span: Span,
}

/// `key: value` in a map literal.
#[derive(Clone, Debug, PartialEq)]
pub struct MapEntry {
    pub key: Expr,
    pub value: Expr,
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// `!`
    Not,
    /// `-`
    Negate,
    /// `~`
    BitNot,
}

impl UnaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnaryOp::Not => "!",
            UnaryOp::Negate => "-",
            UnaryOp::BitNot => "~",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    BitOr,
    BitXor,
    BitAnd,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Or => "||",
            BinaryOp::And => "&&",
            BinaryOp::Equal => "==",
            BinaryOp::NotEqual => "!=",
            BinaryOp::Less => "<",
            BinaryOp::LessEqual => "<=",
            BinaryOp::Greater => ">",
            BinaryOp::GreaterEqual => ">=",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::BitAnd => "&",
            BinaryOp::ShiftLeft => "<<",
            BinaryOp::ShiftRight => ">>",
            BinaryOp::ShiftRightUnsigned => ">>>",
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Remainder => "%",
        }
    }
}

/// The three functional collection operations.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FunctionalOp {
    Map,
    Filter,
    Reduce,
}

impl FunctionalOp {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "map" => Some(FunctionalOp::Map),
            "filter" => Some(FunctionalOp::Filter),
            "reduce" => Some(FunctionalOp::Reduce),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FunctionalOp::Map => "map",
            FunctionalOp::Filter => "filter",
            FunctionalOp::Reduce => "reduce",
        }
    }
}

impl Expr {
    /// Вид узла; оставшаяся оболочка освобождается без детей.
    pub fn into_kind(mut self) -> ExprKind {
        std::mem::replace(&mut self.kind, ExprKind::SelfValue)
    }
}

/// Освобождение без рекурсии (H-03): цепочки `a + a + … + a` и `x.a.b.c…`
/// парсер строит циклом, и дерево получается глубиной в длину цепочки, а
/// рекурсивное освобождение `Box<Expr>` переполнило бы стек. Дочерние
/// выражения переносятся в очередь; блоки внутри выражений освобождаются
/// обычным путём — их вложенность ограничена рекурсией парсера.
impl Drop for Expr {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.kind.take_children(&mut pending);
        while let Some(mut expr) = pending.pop() {
            expr.kind.take_children(&mut pending);
        }
    }
}

impl ExprKind {
    /// Переносит дочерние выражения в `out`, оставляя на их месте листья.
    fn take_children(&mut self, out: &mut Vec<Expr>) {
        let mut take = |expr: &mut Box<Expr>| {
            out.push(std::mem::replace(&mut **expr, Expr { kind: ExprKind::SelfValue, span: Span::default() }));
        };
        match self {
            ExprKind::Grouping(operand)
            | ExprKind::Unary { operand, .. }
            | ExprKind::Unpack { operand, .. }
            | ExprKind::Get { object: operand, .. }
            | ExprKind::Select { object: operand, .. }
            | ExprKind::Projection { construction: operand, .. }
            | ExprKind::Panic { value: operand }
            | ExprKind::Await { value: operand }
            | ExprKind::Spawn { call: operand }
            | ExprKind::If { condition: operand, .. } => take(operand),
            ExprKind::Binary { left, right, .. }
            | ExprKind::StreamWrite { sink: left, source: right, .. }
            | ExprKind::Range { start: left, end: right, .. } => {
                take(left);
                take(right);
            }
            ExprKind::Index { object, index, fallback } => {
                take(object);
                take(index);
                if let Some(fallback) = fallback {
                    take(fallback);
                }
            }
            ExprKind::Call { callee, args } => {
                take(callee);
                out.extend(args.drain(..).map(|arg| arg.value));
            }
            ExprKind::Functional { receiver, args, .. } => {
                take(receiver);
                out.extend(args.drain(..).map(|arg| arg.value));
            }
            ExprKind::ArrayCast { args, .. } => out.extend(args.drain(..).map(|arg| arg.value)),
            ExprKind::Array { elements } => out.append(elements),
            ExprKind::Map { entries } => {
                for entry in entries.drain(..) {
                    out.push(entry.key);
                    out.push(entry.value);
                }
            }
            ExprKind::Literal(_)
            | ExprKind::Name(_)
            | ExprKind::SelfValue
            | ExprKind::OwnerField { .. }
            | ExprKind::AnonymousAction { .. }
            | ExprKind::Match(_)
            | ExprKind::Loop { .. }
            | ExprKind::Try(_) => {}
        }
    }
}

