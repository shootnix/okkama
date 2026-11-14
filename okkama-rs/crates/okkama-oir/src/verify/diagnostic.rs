use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleId { S1, S2, S3, T1, T2, T3, T4, T5, T6 }

impl RuleId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S1 => "OIR2-S1",
            Self::S2 => "OIR2-S2",
            Self::S3 => "OIR2-S3",
            Self::T1 => "OIR2-T1",
            Self::T2 => "OIR2-T2",
            Self::T3 => "OIR2-T3",
            Self::T4 => "OIR2-T4",
            Self::T5 => "OIR2-T5",
            Self::T6 => "OIR2-T6",
        }
    }
}
impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSpan { pub start: usize, pub end: usize }
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VerifyEdgeId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationSite {
    Instruction { function: FuncId, instruction: InstId },
    Terminator { function: FuncId, block: BlockId },
}
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperandPath(pub Vec<usize>);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityLocation {
    Module,
    Declaration(DeclId),
    Summary(SummaryId),
    Protocol(ProtocolId),
    Type(TypeId),
    Scope(ScopeId),
    Function(FuncId),
    Block { function: FuncId, block: BlockId },
    Instruction { function: FuncId, instruction: InstId },
    Terminator { function: FuncId, block: BlockId },
    Operand { site: OperationSite, path: OperandPath },
    Edge { function: FuncId, edge: VerifyEdgeId },
    Value { function: FuncId, value: ValueId },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrLocation {
    pub entity: EntityLocation,
    pub text_span: Option<ByteSpan>,
    pub source_loc: Option<DebugLoc>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticLabel { pub location: IrLocation, pub message: String }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowWitness { pub blocks: Vec<BlockId> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyDiagnostic {
    pub rule: RuleId,
    pub primary: IrLocation,
    pub related: Vec<DiagnosticLabel>,
    pub message: String,
    pub witness: Option<FlowWitness>,
}

/// Optional side table supplied by a codec or editor. Source locations remain independent.
pub trait TextLocations {
    fn span(&self, entity: &EntityLocation) -> Option<ByteSpan>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyFailure { Rejected, Incomplete }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyReport {
    pub failure: VerifyFailure,
    pub diagnostics: Vec<VerifyDiagnostic>,
    pub suppressed: usize,
    pub stats: super::VerifyStats,
}

impl std::fmt::Display for VerifyReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {} verifier diagnostics", self.failure, self.diagnostics.len())
    }
}
impl std::error::Error for VerifyReport {}
