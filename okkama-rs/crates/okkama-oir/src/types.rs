use std::collections::HashMap;

use crate::{Arena, ArenaError, DeclId, FuncId, ScopeId, TypeId};

/// Interned value types. `void` is an absent result, never an SSA type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Bool,
    Byte,
    Float,
    Rat,
    Char,
    Str,
    Range,
    Any,
    Never,
    Object { decl: DeclId, args: Vec<TypeId> },
    Role { decl: DeclId, args: Vec<TypeId> },
    Enum(DeclId),
    Variant(DeclId),
    Handle(DeclId),
    Cap { decl: DeclId, args: Vec<TypeId> },
    Successor(TypeId),
    Array(TypeId),
    Map(TypeId, TypeId),
    Builder(TypeId),
    MapBuilder(TypeId, TypeId),
    Fn(FnType),
    Future(Option<TypeId>),
    Parameter(TypeParameterRef),
    Descriptor,
    Frame(FuncId),
    /// The low-level coroutine cell, distinct from language Cell declarations.
    LowCell(TypeId),
    Service(ServiceType),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnType {
    pub params: Vec<TypeId>,
    pub result: Option<TypeId>,
    pub is_async: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServiceType {
    Own,
    Deps,
    Scope(ScopeId),
    Pending,
    Closed,
    UniqueCache,
    Failure,
    /// Optional accumulated failure inside cleanup; not a failure view.
    FailureState,
    /// Optional pending exit after cleanup; preserves an empty state explicitly.
    ExitState,
    StableKey,
    Owner,
    Holder,
    OwnerSnapshot,
    CacheState,
    CarrierState,
    TaskState,
    Pin,
    Receipt,
    CleanupState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GenericOwner {
    Declaration(DeclId),
    Function(FuncId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeParameterRef {
    pub owner: GenericOwner,
    pub index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeParameter {
    pub name: String,
    pub bounds: Vec<TypeId>,
}

/// Types are immutable after interning, so their hash keys and IDs remain stable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeTable {
    types: Arena<TypeId, Type>,
    interned: HashMap<Type, TypeId>,
}

impl TypeTable {
    pub fn new() -> Self { Self::default() }

    pub fn intern(&mut self, ty: Type) -> Result<TypeId, ArenaError> {
        if let Some(&id) = self.interned.get(&ty) {
            return Ok(id);
        }
        let id = self.types.alloc(ty.clone())?;
        self.interned.insert(ty, id);
        Ok(id)
    }

    pub fn get(&self, id: TypeId) -> Result<&Type, ArenaError> { self.types.get(id) }
    pub fn len(&self) -> usize { self.types.len() }
    pub fn is_empty(&self) -> bool { self.types.is_empty() }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &Type)> { self.types.iter() }
}

impl ServiceType {
    /// SSA holders and permissions that must be forwarded or consumed exactly
    /// once per path. Failure/deps/snapshot views and closed evidence are copies.
    pub const fn is_linear(self) -> bool {
        matches!(self, Self::Own | Self::Scope(_) | Self::Pending | Self::UniqueCache |
            Self::CacheState | Self::CarrierState | Self::TaskState | Self::Pin |
            Self::Receipt | Self::CleanupState | Self::ExitState)
    }
}
