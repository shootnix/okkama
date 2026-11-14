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
    /// Proven pointer-free for every definition of this SPB register (#535).
    pub(crate) pointer_free: Vec<bool>,
    pub result: Option<Kind>,
    /// See [`is_driver`].
    pub driver: bool,
}

#[derive(Debug)]
pub struct Loaded {
    pub program: Program,
    pub functions: Vec<FunctionInfo>,
    /// Per descriptor: is it an Event cell (created by `Event.new`)?
    events: Vec<bool>,
    /// Worker Future functions by result type (see [`WORKER`]).
    workers: Vec<(Option<TypeRef>, usize)>,
}

/// Intrinsic name of the synthetic async functions whose frames are the
/// Futures of `worker.spawn`, one per result type (`worker` drives them).
pub const WORKER: &str = "sparrow.worker.process";

/// Append a worker Future function for every result type `worker.spawn`
/// produces in `program`.
fn add_workers(program: &mut Program) -> Vec<(Option<TypeRef>, usize)> {
    let mut results = Vec::new();
    for function in &program.functions {
        for instruction in &function.code {
            if let okkama_sparrow::Instruction::WorkerSpawn { dst, .. } = instruction
                && let Some(Type::Future(result)) = program.ty(function.registers[dst.0.0 as usize])
                    && !results.contains(result) {
                        results.push(*result);
                    }
        }
    }
    results
        .into_iter()
        .map(|result| {
            program.functions.push(okkama_sparrow::Function {
                symbol: format!("$worker#{}", program.functions.len()),
                is_async: true,
                native: None,
                intrinsic: Some(okkama_sparrow::Intrinsic { name: WORKER.into(), objects: Vec::new(), helpers: Vec::new() }),
                registers: Vec::new(),
                params: Vec::new(),
                result,
                code: Vec::new(),
                handlers: Vec::new(),
            });
            (result, program.functions.len() - 1)
        })
        .collect()
}

/// An async native or intrinsic function: its Future is a frame run by a Rust
/// driver (`drivers`), which keeps its state in one extra traced word.
pub fn is_driver(function: &okkama_sparrow::Function) -> bool {
    function.is_async && (function.native.is_some() || function.intrinsic.is_some())
}

impl Loaded {
    pub fn new(mut program: Program) -> Self {
        let workers = add_workers(&mut program);
        // One switch for both JIT roots and suspended-frame layouts. Read it
        // once at load time; never change a live runtime's tracing contract.
        let pointer_free = std::env::var_os("SPARROW_CORE_POINTER_FREE").is_none_or(|value| value != "0");
        let profile = std::env::var("SPARROW_CORE_PROFILE_COMPILE").is_ok_and(|value| !value.is_empty() && value != "0");
        let functions = program
            .functions
            .iter()
            .map(|f| {
                let kinds: Vec<Kind> = f.registers.iter().map(|t| Kind::of(&program, *t)).collect();
                // This work happens at load time, outside jit::compile's timer.
                let started = profile.then(std::time::Instant::now);
                let pointer_free = if pointer_free { crate::pointer_free::analyze(f, &kinds) } else { vec![false; kinds.len()] };
                if let Some(started) = started {
                    eprintln!("; pointer-free analysis\t{}\t{} microseconds", f.symbol, started.elapsed().as_micros());
                }
                FunctionInfo {
                    kinds,
                    pointer_free,
                    result: f.result.map(|t| Kind::of(&program, t)),
                    driver: is_driver(f),
                }
            })
            .collect();
        // A cell is what its constructor intrinsic makes, whatever its name.
        let mut events = vec![false; program.objects.len()];
        for f in &program.functions {
            if let Some(intrinsic) = &f.intrinsic
                && matches!(intrinsic.name.as_str(), "okkama.event.Event.new" | "okkama.event.Event.new_ready")
                    && let Some(object) = intrinsic.objects.first() {
                        events[object.0 as usize] = true;
                    }
        }
        Self { program, functions, events, workers }
    }

    /// The worker Future function for results of type `result`.
    pub fn worker(&self, result: Option<TypeRef>) -> Option<usize> {
        self.workers.iter().find(|(r, _)| *r == result).map(|&(_, function)| function)
    }

    /// Is `descriptor` an Event cell (the object `Event.new` creates)?
    pub fn is_event(&self, descriptor: usize) -> bool {
        self.events.get(descriptor).copied().unwrap_or(false)
    }

    /// The value kind of an `Atomic<T>` descriptor (from its constructor).
    pub fn atomic_value(&self, descriptor: usize) -> Option<Kind> {
        let p = &self.program;
        p.functions.iter().zip(&self.functions).find_map(|(f, info)| {
            let intrinsic = f.intrinsic.as_ref()?;
            (intrinsic.name == "okkama.atomic.Atomic.new_value"
                && intrinsic.objects.first().is_some_and(|o| o.0 as usize == descriptor))
            .then(|| f.params.first().map(|p| info.kinds[p.0 as usize]))
            .flatten()
        })
    }

    /// Field representations for the collector. Async frames keep every
    /// register after the fixed frame words, an optional driver slot, and
    /// retained arguments independent of reused registers (see `tasks`).
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
                .zip(&p.functions)
                .map(|(f, source)| {
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
                    layout.extend(f.kinds.iter().zip(&f.pointer_free).map(|(k, free)| if *free { Repr::Raw } else { k.repr() }));
                    if f.driver {
                        layout.push(Repr::Ref);
                    }
                    layout.extend(source.params.iter().map(|param| f.kinds[param.0 as usize].repr()));
                    layout
                })
                .collect(),
        }
    }
}
