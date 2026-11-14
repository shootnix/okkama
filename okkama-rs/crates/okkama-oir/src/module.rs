use crate::{Arena, Body, DeclId, FileId, FuncId, Profile, ScopeId, TypeId, TypeParameter, TypeTable};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    /// Source module version, not the OIR format version.
    pub version: Option<String>,
    pub profile: Profile,
    pub requires: Vec<Feature>,
    pub imports: Vec<Import>,
    pub declarations: Arena<DeclId, Declaration>,
    pub types: TypeTable,
    pub scopes: Arena<ScopeId, ScopeDecl>,
    pub functions: Arena<FuncId, Function>,
    pub entry: Option<FuncId>,
    /// `None` denotes a production module; `Some([])` is an empty test module.
    pub tests: Option<Vec<TestDecl>>,
    pub files: Arena<FileId, SourceFileDecl>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature { Core, Async, Process, Test }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: String,
    pub major: Option<u32>,
    pub alias: Option<String>,
    pub take: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestDecl {
    pub name: String,
    pub function: FuncId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFileDecl {
    pub path: String,
}

/// Source Okkama location; line and column are one-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DebugLoc {
    pub file: FileId,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub type_params: Vec<TypeParameter>,
    pub kind: DeclarationKind,
    pub loc: Option<DebugLoc>,
}

/// Ordered fields, cases and witness slots must retain declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    Object {
        fields: Vec<Field>,
        witnesses: Vec<Witness>,
        destroy: Option<FuncId>,
        builtin: bool,
    },
    Role { methods: Vec<RoleMethod>, builtin: bool },
    Enum { cases: Vec<String>, is_error: bool },
    Variant { cases: Vec<VariantCase>, is_error: bool },
    Handle,
    Cap { owner: TypeId, fields: Vec<ComputedField> },
    Global { ty: TypeId, init: FuncId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
    pub default: Option<FuncId>,
    pub once: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness {
    pub role: TypeId,
    pub slots: Vec<FuncId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleMethod {
    pub name: String,
    pub signature: Signature,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantCase {
    pub name: String,
    pub layout: DeclId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedField {
    pub name: String,
    pub function: FuncId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub symbol: String,
    pub signature: Signature,
    pub attrs: Vec<FunctionAttr>,
    pub type_params: Vec<TypeParameter>,
    pub body: Option<Body>,
    pub loc: Option<DebugLoc>,
}

/// Function arguments are the entry block parameters, not separate definitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub params: Vec<TypeId>,
    /// Includes explicit service results; an empty vector means `void`.
    pub results: Vec<TypeId>,
    pub ownership_abi: OwnershipAbi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnershipAbi {
    Unelaborated,
    /// Zero-based packet positions in `Signature.params` and `Signature.results`.
    Explicit { input_packet: u16, output_packet: u16 },
    Observation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FunctionAttr {
    Entry,
    Method(DeclId),
    Ctor(DeclId),
    Successor(DeclId),
    Default { object: DeclId, field: u32 },
    Destroy(DeclId),
    Extern(String),
    Hidden,
    Intrinsic { name: String, objects: Vec<DeclId>, helpers: Vec<FuncId> },
    Async,
    Lambda { captures: u32, owner: Option<DeclId> },
}

/// Scope IDs are module-wide, even though each scope belongs to one function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeDecl {
    pub function: FuncId,
    pub parent: Option<ScopeId>,
    pub kind: ScopeKind,
    pub loc: Option<DebugLoc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScopeKind { Action, Block, Branch, Iteration, Try, Catch, Defer }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingDecl {
    pub name: String,
    /// Rebinding preserves the original binding's lexical cleanup boundary.
    pub scope: ScopeId,
    pub ty: TypeId,
    pub loc: Option<DebugLoc>,
}
