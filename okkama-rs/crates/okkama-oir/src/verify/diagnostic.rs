use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleId {
    S1, S2, S3,
    T1, T2, T3, T4, T5, T6,
    SSA1, SSA2,
    A1, A2, A3,
    B51, B61, B62,
    B11, B12, B21, B22, B23, B31, B32, B41, B42, B43, B52, B53, B54, B63, B81, B82, B101, B114,
    H1, H2, H3, H4, H5, H6, H7, H8, H9,
}

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
            Self::SSA1 => "OIR2-SSA1",
            Self::SSA2 => "OIR2-SSA2",
            Self::A1 => "OIR2-A1",
            Self::A2 => "OIR2-A2",
            Self::A3 => "OIR2-A3",
            Self::B11 => "OIR2-B1.1",
            Self::B12 => "OIR2-B1.2",
            Self::B21 => "OIR2-B2.1",
            Self::B22 => "OIR2-B2.2",
            Self::B23 => "OIR2-B2.3",
            Self::B31 => "OIR2-B3.1",
            Self::B32 => "OIR2-B3.2",
            Self::B41 => "OIR2-B4.1",
            Self::B42 => "OIR2-B4.2",
            Self::B43 => "OIR2-B4.3",
            Self::B52 => "OIR2-B5.2",
            Self::B53 => "OIR2-B5.3",
            Self::B54 => "OIR2-B5.4",
            Self::B63 => "OIR2-B6.3",
            Self::B81 => "OIR2-B8.1",
            Self::B82 => "OIR2-B8.2",
            Self::B114 => "OIR2-B11.4",
            Self::B101 => "OIR2-B10.1",
            Self::B51 => "OIR2-B5.1",
            Self::B61 => "OIR2-B6.1",
            Self::B62 => "OIR2-B6.2",
            Self::H1 => "OIR2-H1",
            Self::H2 => "OIR2-H2",
            Self::H3 => "OIR2-H3",
            Self::H4 => "OIR2-H4",
            Self::H5 => "OIR2-H5",
            Self::H6 => "OIR2-H6",
            Self::H7 => "OIR2-H7",
            Self::H8 => "OIR2-H8",
            Self::H9 => "OIR2-H9",

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
