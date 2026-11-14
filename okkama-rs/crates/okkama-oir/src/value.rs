use crate::{Arena, BindingDecl, BindingId, BlockId, DebugLoc, InstId, Operation, ScopeId, TypeId, ValueId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Body {
    pub cleanup: crate::CleanupMetadata,
    pub high_results: Vec<HighResultExit>,
    pub high_effects: Vec<HighEffectRecord>,
    pub descriptor_bindings: Vec<crate::DescriptorBinding>,
    pub service_schedules: Vec<crate::ServiceSchedule>,
    pub entry: BlockId,
    pub values: Arena<ValueId, ValueData>,
    pub blocks: Arena<BlockId, Block>,
    pub insts: Arena<InstId, Instruction>,
    pub bindings: Arena<BindingId, BindingDecl>,
    pub boundaries: Arena<crate::BoundaryId, crate::Boundary>,
    pub deps: Arena<crate::DepExprId, crate::DepExpr>,
    pub keys: Arena<crate::KeyExprId, crate::KeyExpr>,
    pub selections: Arena<crate::SelectionId, crate::SelectionExpr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueData {
    pub ty: TypeId,
    pub def: Definition,
    pub facts: crate::ValueFacts,
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
    pub facts: crate::InstructionFacts,
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
pub enum Outcome {
    Normal, Unwind, Cancel, DeferContinue, DeferReplace(u16),
    ExitSuccess, ExitPending, ExitFailure, FailureSome, CleanupSelected, CleanupDone,
}

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

/// Edge results are interpreted with the invoked operation contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermKind {
    /// Runtime serial selector. Selection never executes destroy.
    CleanupNext { cleanup: ValueId, selected: InvokeEdge, done: InvokeEdge },
    ExitDispatch {
        close: crate::CleanupCloseId,
        closed: ValueId,
        failure: ValueId,
        exit: ValueId,
        success: Option<InvokeEdge>,
        pending: Option<InvokeEdge>,
        failed: Option<InvokeEdge>,
    },
    FailureDispatch { failure: ValueId, none: Target, some: InvokeEdge },
    Br(Target),
    Cond { condition: ValueId, yes: Target, no: Target },
    Switch { value: ValueId, cases: Vec<SwitchCase>, default: Target },
    InvokeOwn { op: Box<Operation>, normal: InvokeEdge, unwind: InvokeEdge },
    InvokeDefer {
        function: crate::FuncId,
        call: crate::CallSite,
        continue_edge: InvokeEdge,
        replacements: Vec<DeferReplaceEdge>,
        unwind: Option<InvokeEdge>,
    },
    RetDefer { tag: DeferReturnTag, values: Vec<ValueId> },
    Ret { values: Vec<ValueId> },
    Throw(ValueId),
    Raise(ValueId),
    Panic(ValueId),
    Trap { code: crate::TrapCode, message: String },
    Catch { failure: ValueId, ty: TypeId, handler: InvokeEdge, next: Target },
    Lookup { kind: LookupKind, collection: ValueId, key: ValueId, found: InvokeEdge, missing: Target },
    Unreachable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchCase {
    pub value: crate::Literal,
    pub target: Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupKind { Array, Map }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapCode { Confirm, Range, Empty, Unbound, Unreachable, Pending }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeferReplaceEdge {
    pub label: String,
    pub edge: InvokeEdge,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeferReturnTag { Continue, Replace(String) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighEdgeSelector {
    Normal(u16),
    Unwind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighResultExit {
    pub block: BlockId,
    pub edge: HighEdgeSelector,
    pub value: ValueId,
    pub demand: crate::ResultDemand,
    pub prepared: Option<InstId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighOperationSite { Instruction(InstId), Terminator(BlockId) }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HighEffectEnvelope {
    pub failures: crate::FailureKindSet,
    pub may_suspend: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighEffectRecord {
    pub site: HighOperationSite,
    pub effects: HighEffectEnvelope,
}
