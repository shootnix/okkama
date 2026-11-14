use crate::{Arena, BindingDecl, BindingId, BlockId, DebugLoc, InstId, Operation, ScopeId, TypeId, ValueId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub entry: BlockId,
    pub values: Arena<ValueId, ValueData>,
    pub blocks: Arena<BlockId, Block>,
    pub insts: Arena<InstId, Instruction>,
    pub bindings: Arena<BindingId, BindingDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueData {
    pub ty: TypeId,
    pub def: Definition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Definition {
    BlockParam { block: BlockId, index: u32 },
    InstResult { inst: InstId, index: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub results: Vec<ValueId>,
    pub op: Operation,
    pub loc: Option<DebugLoc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub lexical_scope: ScopeId,
    pub params: Vec<ValueId>,
    pub unwind: Option<BlockUnwind>,
    pub insts: Vec<InstId>,
    pub term: Terminator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub block: BlockId,
    pub args: Vec<ValueId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockUnwind {
    /// The implicit failure is the target's last parameter, after these arguments.
    pub target: Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome { Normal, Unwind }

/// An invoke result belongs only to its edge, never to the source block's SSA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeResult {
    pub outcome: Outcome,
    pub slot: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeArg {
    Value(ValueId),
    Produced(EdgeResult),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvokeEdge {
    pub block: BlockId,
    pub args: Vec<EdgeArg>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terminator {
    pub kind: TermKind,
    pub loc: Option<DebugLoc>,
}

/// Structural control flow only; operation outcome schemas arrive in commit 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermKind {
    Br(Target),
    Cond { condition: ValueId, yes: Target, no: Target },
    Switch { value: ValueId, cases: Vec<SwitchCase>, default: Target },
    InvokeOwn { op: Box<Operation>, normal: InvokeEdge, unwind: InvokeEdge },
    Ret { values: Vec<ValueId> },
    Throw(ValueId),
    Raise(ValueId),
    Panic(ValueId),
    Unreachable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchCase {
    pub value: crate::Literal,
    pub target: Target,
}
