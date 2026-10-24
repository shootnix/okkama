//! Type expressions and action result specifications.

use super::Name;
use crate::source::Span;

/// A type expression as written in the source.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    /// `int`, `Cash`, `io.Package`, `Map<string, Item>`.
    ///
    /// `path` has one segment for a plain name and more for a qualified
    /// name; `args` is empty unless a `<...>` list was written (the grammar
    /// requires at least one argument in a written list).
    Named {
        path: Vec<Name>,
        args: Vec<TypeExpr>,
    },
    /// `Owner::Cap` — a cap type nested in its owner type.
    Nested { owner: Box<TypeExpr>, name: Name },
    /// `@T`
    Array(Box<TypeExpr>),
    /// `%K:V`
    Map {
        key: Box<TypeExpr>,
        value: Box<TypeExpr>,
    },
    /// `(A, B) is R`, `() is void`, `async (A) is R`.
    Action {
        is_async: bool,
        params: Vec<TypeExpr>,
        result: ActionResult,
    },
}

/// The result of an action type or an anonymous action: a type or `void`.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionResult {
    /// `void` (the span covers the word).
    Void(Span),
    Type(Box<TypeExpr>),
}

/// The result specification after `is` in a named action, role signature or
/// native declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum ResultSpec {
    /// `is void`
    Void(Span),
    /// `is ^Type` — a successor action.
    Successor { ty: TypeExpr, span: Span },
    /// `is Type`
    Type(TypeExpr),
}

impl ResultSpec {
    pub fn span(&self) -> Span {
        match self {
            ResultSpec::Void(span) => *span,
            ResultSpec::Successor { span, .. } => *span,
            ResultSpec::Type(ty) => ty.span,
        }
    }
}
