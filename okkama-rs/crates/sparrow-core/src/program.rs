//! Static facts about a loaded SPB program: the machine representation of every
//! register and field, heap layouts and the lazily filled code table.
use crate::heap::Layouts;
use crate::word::Repr;
use okkama_sparrow::{Program, Type, TypeRef};

/// Machine kind of a register: how the JIT holds it and how the collector sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Tagged small integer or BigInt pointer.
    Int,
    /// F64 value; stored as raw bits.
    Float,
    /// bool, byte, char.
    Raw,
    /// Heap object pointer.
    Ref,
}

impl Kind {
    pub fn of(program: &Program, ty: TypeRef) -> Self {
        match program.ty(ty) {
            Some(Type::Int) => Self::Int,
            Some(Type::Float) => Self::Float,
            // Payload-less enums are `(enum << 32) | case`.
            Some(Type::Bool | Type::Byte | Type::Char | Type::Enum(_)) => Self::Raw,
            _ => Self::Ref,
        }
    }
    pub fn repr(self) -> Repr {
        match self {
            Self::Int => Repr::Int,
            Self::Float | Self::Raw => Repr::Raw,
            Self::Ref => Repr::Ref,
        }
    }
    /// Values the collector must see in stack maps.
    pub fn traced(self) -> bool {
        matches!(self, Self::Int | Self::Ref)
    }
}

#[derive(Debug)]
pub struct FunctionInfo {
    pub kinds: Vec<Kind>,
    pub result: Option<Kind>,
    /// See [`is_driver`].
    pub driver: bool,
}

#[derive(Debug)]
pub struct Loaded {
    pub program: Program,
    pub functions: Vec<FunctionInfo>,
}

/// An async native or intrinsic function: its Future is a frame run by a Rust
/// driver (`drivers`), which keeps its state in one extra traced word.
pub fn is_driver(function: &okkama_sparrow::Function) -> bool {
    function.is_async && (function.native.is_some() || function.intrinsic.is_some())
}

impl Loaded {
    pub fn new(program: Program) -> Self {
        let functions = program
            .functions
            .iter()
            .map(|f| FunctionInfo {
                kinds: f.registers.iter().map(|t| Kind::of(&program, *t)).collect(),
                result: f.result.map(|t| Kind::of(&program, t)),
                driver: is_driver(f),
            })
            .collect();
        Self { program, functions }
    }

    /// Field representations for the collector. Async frames keep every
    /// register after the fixed frame words (see `tasks`).
    pub fn layouts(&self) -> Layouts {
        let p = &self.program;
        Layouts {
            objects: p
                .objects
                .iter()
                .map(|o| o.fields.iter().map(|(_, t)| Kind::of(p, *t).repr()).collect())
                .collect(),
            // A closure object has one word per parameter of its function:
            // captures fill a prefix, the rest stay unbound.
            closures: p
                .functions
                .iter()
                .zip(&self.functions)
                .map(|(f, info)| {
                    f.params
                        .iter()
                        .map(|r| info.kinds[r.0 as usize].repr())
                        .collect()
                })
                .collect(),
            frames: self
                .functions
                .iter()
                .map(|f| {
                    // state, result, failure, waiter, awaiting, resume, flags
                    let mut layout = vec![
                        Repr::Raw,
                        f.result.map_or(Repr::Raw, Kind::repr),
                        Repr::Ref,
                        Repr::Ref,
                        Repr::Ref,
                        Repr::Raw,
                        Repr::Raw,
                    ];
                    layout.extend(f.kinds.iter().map(|k| k.repr()));
                    if f.driver {
                        layout.push(Repr::Ref);
                    }
                    layout
                })
                .collect(),
        }
    }
}
