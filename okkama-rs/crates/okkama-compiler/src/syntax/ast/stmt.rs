//! Statements and blocks.

use super::{Expr, Name, TypeExpr};
use crate::source::Span;
use crate::syntax::token::IntLiteral;

/// `{ statement* }`
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    /// `let name is Type = value;`
    Let {
        name: Name,
        ty: Option<TypeExpr>,
        value: Expr,
    },
    /// `defer expr;`
    Defer { value: Expr },
    /// `try { } catch e is T { } ...` in statement position.
    Try(TryBody),
    /// `throw expr;` or the bare rethrow `throw;`.
    Throw { value: Option<Expr> },
    /// `return expr;` or `return;`.
    Return { value: Option<Expr> },
    /// `confirm cond;` with an optional `else throw expr` / `else panic expr`.
    Confirm {
        condition: Expr,
        fallback: Option<ConfirmFallback>,
    },
    /// `^field = value;`
    SuccessorAssign { name: Name, value: Expr },
    /// A nested `{ ... }` block statement.
    Block(Block),
    /// `if` in statement position (may chain `else if`).
    If(IfStmt),
    /// `match` in statement position.
    Match(MatchBody),
    /// `for source -> a, b { }`
    For {
        source: Expr,
        bindings: Vec<ForBinding>,
        body: Block,
    },
    /// `next;` or `next if cond;`
    Next { condition: Option<Expr> },
    /// `last;`, `last value;`, `last if cond;`, `last value if cond;`
    Last {
        value: Option<Expr>,
        condition: Option<Expr>,
    },
    /// `name = value;` — constructor field bind or local rebinding (which
    /// one is decided by semantic analysis).
    FieldBind { name: Name, value: Expr },
    /// `self.name = value;`
    SelfFieldBind {
        receiver: Span,
        name: Name,
        value: Expr,
    },
    /// `expr;` or `expr if cond;`. A statement-level `loop { }` is also an
    /// expression statement (its `;` is optional).
    Expr { expr: Expr, condition: Option<Expr> },
}

/// The failure form after `confirm cond else`.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmFallback {
    /// `else panic value`
    Panic { keyword: Span, value: Expr },
    /// `else throw value`
    Throw { keyword: Span, value: Expr },
}

/// `if cond { } else if ... else { }` in statement position.
#[derive(Clone, Debug, PartialEq)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElseBranch {
    Block(Block),
    If(Box<IfStmt>),
}

/// The shared body of `match` statements and expressions.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchBody {
    pub subject: Expr,
    pub cases: Vec<MatchCase>,
    pub else_block: Option<Block>,
    pub span: Span,
}

/// `case p1, p2 { }`
#[derive(Clone, Debug, PartialEq)]
pub struct MatchCase {
    pub patterns: Vec<Expr>,
    pub body: Block,
    pub span: Span,
}

/// The shared body of `try` statements and expressions.
#[derive(Clone, Debug, PartialEq)]
pub struct TryBody {
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub span: Span,
}

/// `catch name is Type { }`
#[derive(Clone, Debug, PartialEq)]
pub struct CatchClause {
    pub name: Name,
    pub ty: TypeExpr,
    pub body: Block,
    pub span: Span,
}

/// A `for` binding: `name` or the batched form `name[INT]`.
#[derive(Clone, Debug, PartialEq)]
pub struct ForBinding {
    pub name: Name,
    pub batch: Option<BatchSize>,
    pub span: Span,
}

/// The integer literal inside `name[INT]`.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchSize {
    pub literal: IntLiteral,
    pub span: Span,
}
