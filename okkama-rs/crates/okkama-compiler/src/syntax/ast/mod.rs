//! The typed abstract syntax tree.
//!
//! The AST is a faithful, purely syntactic representation of one source
//! file. It contains no symbol tables, resolved names, computed types or
//! runtime values, and it does not desugar anything: `async`, casts,
//! projections, functional suffixes, `test`/`fake` blocks and similar forms
//! stay exactly as written so that later stages can analyse them.
//!
//! Every node carries the byte [`Span`] of the source text it was parsed
//! from. Names are [`Name`] values (text plus span). Declarations, statements,
//! expressions and type expressions are separate types:
//!
//! * [`Program`], [`Item`] and the declaration structs in this module;
//! * statements in [`stmt`];
//! * expressions in [`expr`];
//! * type expressions in [`types`].

pub mod expr;
pub mod stmt;
pub mod types;

pub use expr::*;
pub use stmt::*;
pub use types::*;

use crate::source::Span;
use crate::syntax::token::StrLiteral;

/// A name token: identifier, type name, contextual keyword-shaped label, ...
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

/// A string literal used in a declaration (import path, test name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringLit {
    pub literal: StrLiteral,
    pub span: Span,
}

/// One parsed source file.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub module: ModuleDecl,
    /// Imports always precede declarations; the grammar enforces the order.
    pub imports: Vec<ImportDecl>,
    pub items: Vec<Item>,
    pub span: Span,
}

/// `module app/api v1.2.3;`
#[derive(Clone, Debug, PartialEq)]
pub struct ModuleDecl {
    /// Path segments (`app`, `api`).
    pub path: Vec<Name>,
    pub version: Version,
    pub span: Span,
}

/// `vMAJOR.MINOR.PATCH`, kept as the source spelling of each component.
///
/// The lexer has no version token: `v1.0.0` arrives as `v1`, `.`, `0.0`
/// (a float). The parser reassembles the components and keeps their exact
/// spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: String,
    pub minor: String,
    pub patch: String,
    pub span: Span,
}

impl Version {
    /// `v1.0.0`
    pub fn text(&self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// `import "path" as alias take a, b;`
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDecl {
    pub path: StringLit,
    pub alias: Option<Name>,
    pub take: Vec<Name>,
    pub span: Span,
}

/// A module-scope declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Object(ObjectDecl),
    Role(RoleDecl),
    Enum(EnumDecl),
    Test(TestDecl),
    NativeAction(NativeActionDecl),
    NativeObject(NativeObjectDecl),
    /// `cap Owner::Name { ... }` declared outside its owner.
    Cap(CapDecl),
    /// The entry point. The grammar allows only `main()` at module scope.
    Action(ActionDecl),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Object(decl) => decl.span,
            Item::Role(decl) => decl.span,
            Item::Enum(decl) => decl.span,
            Item::Test(decl) => decl.span,
            Item::NativeAction(decl) => decl.span,
            Item::NativeObject(decl) => decl.span,
            Item::Cap(decl) => decl.span,
            Item::Action(decl) => decl.span,
        }
    }
}

/// A generic declaration parameter `T` or `T: Bound<T>`.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericParam {
    pub name: Name,
    pub bound: Option<TypeExpr>,
    pub span: Span,
}

/// `object Name<T> does Role, Other { members }`
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub roles: Vec<TypeExpr>,
    pub members: Vec<ObjectMember>,
    pub span: Span,
}

/// A member of an object body (also used for role `fake` bodies).
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectMember {
    Has(HasDecl),
    Cap(CapDecl),
    Constructor(ConstructorDecl),
    Destroy(DestroyDecl),
    Test(TestDecl),
    Action(ActionDecl),
    /// `private:` — every following member of the body is a private action.
    Private(PrivateSection),
}

impl ObjectMember {
    pub fn span(&self) -> Span {
        match self {
            ObjectMember::Has(decl) => decl.span,
            ObjectMember::Cap(decl) => decl.span,
            ObjectMember::Constructor(decl) => decl.span,
            ObjectMember::Destroy(decl) => decl.span,
            ObjectMember::Test(decl) => decl.span,
            ObjectMember::Action(decl) => decl.span,
            ObjectMember::Private(section) => section.span,
        }
    }
}

/// `private: action() is T { } ...` up to the end of the object body.
///
/// The section is kept as a node (rather than only flagging the actions) so
/// the tree shows where it starts; its actions also carry
/// [`Visibility::Private`].
#[derive(Clone, Debug, PartialEq)]
pub struct PrivateSection {
    pub actions: Vec<ActionDecl>,
    pub span: Span,
}

/// `has a, b is Type = default;` — the type and the default are optional.
#[derive(Clone, Debug, PartialEq)]
pub struct HasDecl {
    pub names: Vec<Name>,
    pub ty: Option<TypeExpr>,
    pub default: Option<Expr>,
    pub span: Span,
}

/// `cap Name { field is T { body } ... }` inside an object, or
/// `cap Owner::Name { ... }` at module scope.
#[derive(Clone, Debug, PartialEq)]
pub struct CapDecl {
    pub owner: Option<Name>,
    pub name: Name,
    pub fields: Vec<ComputedField>,
    pub span: Span,
}

/// `name is Type { body }` inside a cap.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedField {
    pub name: Name,
    pub ty: TypeExpr,
    pub body: Block,
    pub span: Span,
}

/// A constructor declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructorDecl {
    pub name: Name,
    pub form: ConstructorForm,
    pub span: Span,
}

/// The three constructor shapes are syntactically distinct and stay so:
/// `Name(a is int) { }` is `Full`, `Name(:a, :b);` / `Name(:a) { }` is
/// `Shorthand`, and `Name();` is a `Shorthand` with no fields (which differs
/// from the full form `Name() {}`).
#[derive(Clone, Debug, PartialEq)]
pub enum ConstructorForm {
    Full {
        params: Vec<Param>,
        body: Block,
    },
    Shorthand {
        fields: Vec<Name>,
        body: Option<Block>,
    },
}

/// `destroy { ... }`
#[derive(Clone, Debug, PartialEq)]
pub struct DestroyDecl {
    pub body: Block,
    pub span: Span,
}

/// `test "name" { }` or `async test "name" { }`.
#[derive(Clone, Debug, PartialEq)]
pub struct TestDecl {
    pub is_async: bool,
    pub name: StringLit,
    pub body: TestBody,
    pub span: Span,
}

/// Test and fake bodies are fully parsed in [`ParseMode::Full`]. In
/// [`ParseMode::Production`] the language skips their inner grammar (only
/// balanced braces are required), and the body is recorded as `Skipped`.
///
/// [`ParseMode::Full`]: crate::syntax::ParseMode::Full
/// [`ParseMode::Production`]: crate::syntax::ParseMode::Production
#[derive(Clone, Debug, PartialEq)]
pub enum TestBody {
    Parsed(Block),
    Skipped(Span),
}

/// Who may call an action.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Private,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
        }
    }
}

/// `#projected` or `#projected(alias)` in front of an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionDirective {
    pub alias: Option<Name>,
    pub span: Span,
}

/// An action with a body.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionDecl {
    pub is_async: bool,
    pub visibility: Visibility,
    pub projection: Option<ProjectionDirective>,
    pub name: Name,
    pub params: Vec<Param>,
    pub result: ResultSpec,
    pub body: Block,
    pub span: Span,
}

/// `name is Type` or the variadic `name is ...Type`.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: Name,
    pub variadic: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

/// `role Name<T> { signatures, fake, tests }`
#[derive(Clone, Debug, PartialEq)]
pub struct RoleDecl {
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub members: Vec<RoleMember>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoleMember {
    Signature(ActionSignature),
    Fake(FakeDecl),
    Test(TestDecl),
}

impl RoleMember {
    pub fn span(&self) -> Span {
        match self {
            RoleMember::Signature(decl) => decl.span,
            RoleMember::Fake(decl) => decl.span,
            RoleMember::Test(decl) => decl.span,
        }
    }
}

/// A bodyless action signature in a role.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionSignature {
    pub is_async: bool,
    pub name: Name,
    pub params: Vec<Param>,
    pub result: ResultSpec,
    pub span: Span,
}

/// `fake { object members }` inside a role.
#[derive(Clone, Debug, PartialEq)]
pub struct FakeDecl {
    pub body: FakeBody,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FakeBody {
    Parsed(Vec<ObjectMember>),
    Skipped(Span),
}

/// `enum Name = (a, b);`
#[derive(Clone, Debug, PartialEq)]
pub struct EnumDecl {
    pub name: Name,
    pub cases: Vec<Name>,
    pub span: Span,
}

/// `native async? name(params) is Result;`
#[derive(Clone, Debug, PartialEq)]
pub struct NativeActionDecl {
    pub is_async: bool,
    pub name: Name,
    pub params: Vec<Param>,
    pub result: ResultSpec,
    pub span: Span,
}

/// `native object Name;`
#[derive(Clone, Debug, PartialEq)]
pub struct NativeObjectDecl {
    pub name: Name,
    pub span: Span,
}
