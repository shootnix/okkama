use crate::*;

// Keep the public inventory and stable names inseparable from the enum.
macro_rules! rule_ids {
    ($($variant:ident => $name:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum RuleId {
            $($variant),*
        }

        impl RuleId {
            /// Complete semantic rule inventory, including textual structural errors.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),*
                }
            }
        }
    };
}

rule_ids! {
    S1 => "OIR2-S1",
    S2 => "OIR2-S2",
    S3 => "OIR2-S3",
    T1 => "OIR2-T1",
    T2 => "OIR2-T2",
    T3 => "OIR2-T3",
    T4 => "OIR2-T4",
    T5 => "OIR2-T5",
    T6 => "OIR2-T6",
    SSA1 => "OIR2-SSA1",
    SSA2 => "OIR2-SSA2",
    A1 => "OIR2-A1",
    A2 => "OIR2-A2",
    A3 => "OIR2-A3",
    B51 => "OIR2-B5.1",
    B61 => "OIR2-B6.1",
    B62 => "OIR2-B6.2",
    B11 => "OIR2-B1.1",
    B12 => "OIR2-B1.2",
    B21 => "OIR2-B2.1",
    B22 => "OIR2-B2.2",
    B23 => "OIR2-B2.3",
    B31 => "OIR2-B3.1",
    B32 => "OIR2-B3.2",
    B41 => "OIR2-B4.1",
    B42 => "OIR2-B4.2",
    B43 => "OIR2-B4.3",
    B52 => "OIR2-B5.2",
    B53 => "OIR2-B5.3",
    B54 => "OIR2-B5.4",
    B63 => "OIR2-B6.3",
    B81 => "OIR2-B8.1",
    B82 => "OIR2-B8.2",
    B101 => "OIR2-B10.1",
    B71 => "OIR2-B7.1",
    B72 => "OIR2-B7.2",
    B73 => "OIR2-B7.3",
    B74 => "OIR2-B7.4",
    B75 => "OIR2-B7.5",
    B91 => "OIR2-B9.1",
    B92 => "OIR2-B9.2",
    B93 => "OIR2-B9.3",
    B94 => "OIR2-B9.4",
    B95 => "OIR2-B9.5",
    B96 => "OIR2-B9.6",
    B102 => "OIR2-B10.2",
    B103 => "OIR2-B10.3",
    B104 => "OIR2-B10.4",
    B105 => "OIR2-B10.5",
    B106 => "OIR2-B10.6",
    B111 => "OIR2-B11.1",
    B112 => "OIR2-B11.2",
    B113 => "OIR2-B11.3",
    B114 => "OIR2-B11.4",
    H1 => "OIR2-H1",
    H2 => "OIR2-H2",
    H3 => "OIR2-H3",
    H4 => "OIR2-H4",
    H5 => "OIR2-H5",
    H6 => "OIR2-H6",
    H7 => "OIR2-H7",
    H8 => "OIR2-H8",
    H9 => "OIR2-H9",
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
