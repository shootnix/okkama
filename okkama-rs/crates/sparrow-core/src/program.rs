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
    /// bool, char, payload-less enums.
    Raw,
    /// byte: raw bits in registers; arrays of bytes are packed (see `heap`).
    Byte,
    /// Heap object pointer.
    Ref,
}

impl Kind {
    pub fn of(program: &Program, ty: TypeRef) -> Self {
        match program.ty(ty) {
            Some(Type::Int) => Self::Int,
            Some(Type::Float) => Self::Float,
            // Payload-less enums are `(enum << 32) | case`.
            Some(Type::Byte) => Self::Byte,
            Some(Type::Bool | Type::Char | Type::Enum(_)) => Self::Raw,
            _ => Self::Ref,
        }
    }
    pub fn repr(self) -> Repr {
        match self {
            Self::Int => Repr::Int,
            Self::Float | Self::Raw | Self::Byte => Repr::Raw,
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
    /// Descriptor of `async.event::Event`, if the program has it.
    event: Option<usize>,
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
        let event = program
            .objects
            .iter()
            .position(|o| o.name == "async.event::Event" && o.fields.len() == 1 && o.fields[0].0 == "initial");
        Self { program, functions, event }
    }

    /// Is `descriptor` the `async.event::Event` object (`initial` only)?
    pub fn is_event(&self, descriptor: usize) -> bool {
        self.event == Some(descriptor)
    }

    /// The value kind of an `Atomic<T>` descriptor (from its constructor).
    pub fn atomic_value(&self, descriptor: usize) -> Option<Kind> {
        let p = &self.program;
        if !p.objects[descriptor].name.starts_with("async.atomic::Atomic<") {
            return None;
        }
        p.functions.iter().zip(&self.functions).find_map(|(f, info)| {
            let intrinsic = f.intrinsic.as_ref()?;
            (intrinsic.name == "okkama.atomic.Atomic.new_value"
                && intrinsic.objects.first().is_some_and(|o| o.0 as usize == descriptor))
            .then(|| info.kinds[f.params[0].0 as usize])
        })
    }

    /// Field representations for the collector. Async frames keep every
    /// register after the fixed frame words (see `tasks`).
    pub fn layouts(&self) -> Layouts {
        let p = &self.program;
        Layouts {
            objects: p
                .objects
                .iter()
                .enumerate()
                .map(|(descriptor, o)| match self.atomic_value(descriptor) {
                    // An Atomic cell: its value, then the closed flag (see `intrinsics`).
                    Some(kind) => vec![kind.repr(), Repr::Raw],
                    // An Event: `initial`, then the set flag and its waiters (`primitives`).
                    None if self.is_event(descriptor) => vec![Repr::Raw, Repr::Raw, Repr::Ref],
                    None => o.fields.iter().map(|(_, t)| Kind::of(p, *t).repr()).collect(),
                })
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
