//! Semantic cleanup schemas. These IDs are function-local, never wire/runtime offsets.
use crate::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupMetadata {
    pub closes: Arena<CleanupCloseId, CleanupClose>,
    pub schedules: Arena<CleanupScheduleId, CleanupSchedule>,
    pub plans: Arena<CleanupPlanId, CleanupPlan>,
    pub roots: Arena<RootMapId, RootRequirement>,
    pub proofs: Arena<CleanupOrderProofId, CleanupOrderProof>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CleanupSource { Scope(ScopeId), Packet(BoundaryId) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupClose {
    pub source: CleanupSource,
    /// Entry after the already elaborated reached defer prefix. No runtime defer runner.
    pub after_defers: BlockId,
    pub reserve: bool,
    pub active_failure: bool,
    pub effects: ScopeEffectEnvelope,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupSchedule {
    pub close: CleanupCloseId,
    /// The complete D16 inventory. L1 admits only the checked empty inventory.
    pub holders: Vec<ValueId>,
    /// The checked Services protocol/dictionary for a nonempty owner duty inventory.
    /// A finite empty P1 inventory needs neither runtime operand.
    pub dictionary: Option<ValueId>,
    pub protocol: Option<ProtocolId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeSource { ScopeInventory, PacketInventory, WholeReserveOnFailure }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupPlan {
    pub close: CleanupCloseId,
    /// Complete inventory in either form. Without a direct proof, one serial selector covers every source.
    /// Reserve joins only the unexecuted suffix.
    pub sources: Vec<MergeSource>,
    /// None selects the common merge executor; Some selects the checked finite direct chain.
    pub direct: Option<CleanupOrderProofId>,
}
/// A completion event, never an allocation, admission order or a fabricated serial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreationWitness {
    /// No relative serial order is needed for one independently checked identity.
    Singleton,
    /// Successful complete obj.new or completion-bearing obj.init in the current OIR vocabulary.
    Create { site: HighOperationSite },
    FreshResult { site: HighOperationSite, callee: FuncId, summary: SummaryId },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectSlot {
    pub identity: ValueId,
    pub descriptor: ValueId,
    pub destroy: FuncId,
    pub origin: CreationWitness,
    /// This identity is unconditionally present at this close, in this scope instance.
    pub active_at: BlockId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreserveWitness {
    pub from: ValueId,
    pub to: ValueId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinWitness {
    pub state: ValueId,
    pub predecessors: Vec<ValueId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupSuffix {
    /// Number of completed (Dead) entries, including current. Both outcomes use this suffix.
    pub completed: u32,
    pub remaining: Vec<ValueId>,
    pub roots: Vec<ValueId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupOrderProof {
    pub state: ValueId,
    pub slots: Vec<DirectSlot>,
    pub preserves: Vec<PreserveWitness>,
    pub joins: Vec<JoinWitness>,
    pub suffixes: Vec<CleanupSuffix>,
    /// B2: no pending, or a pending whose entire reserve is checked empty.
    pub reserve_empty: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootBoundary { BeforeInstruction(InstId), AfterInstruction(InstId), Terminator(BlockId), Entry(BlockId) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootTrace {
    Value(TypeId), Descriptor(ValueId), FullDependencies,
    /// Current self and traced fields, static/admission/history/staging suffix, reserve and failures.
    CleanupActivation,
    ScopeInventory, PacketInventory, PendingValueReserveDeps, FailureGraph, ExitReserve,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootBinding { pub value: ValueId, pub trace: RootTrace }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootRequirement {
    pub boundary: RootBoundary,
    pub bindings: Vec<RootBinding>,
    pub through_suspend: bool,
}

/// The schemas are independent of the producer and shared by codec, builder and verifier.
pub fn dispatch_slots(outcome: Outcome) -> Vec<ResultSlot> {
    use ServiceType as S;
    use SlotName as N;
    match outcome {
        Outcome::CleanupSelected => vec![ResultSlot::service(N::State, S::CleanupState),
            ResultSlot::service(N::Selected, S::StableKey)],
        Outcome::CleanupDone => vec![ResultSlot::service(N::State, S::CleanupState)],
        Outcome::ExitSuccess => vec![ResultSlot::service(N::Closed, S::Closed)],
        Outcome::ExitPending => vec![ResultSlot::service(N::Closed, S::Closed),
            ResultSlot::service(N::Pending, S::Pending)],
        Outcome::ExitFailure => vec![ResultSlot::service(N::Closed, S::Closed),
            ResultSlot::service(N::Failure, S::Failure)],
        Outcome::FailureSome => vec![ResultSlot::service(N::Failure, S::Failure)],
        _ => vec![],
    }
}
pub fn dispatch_edges(term: &TermKind) -> Vec<(Outcome, &InvokeEdge)> {
    match term {
        TermKind::CleanupNext { selected, done, .. } => {
            vec![(Outcome::CleanupSelected, selected), (Outcome::CleanupDone, done)]
        }
        TermKind::ExitDispatch { success, pending, failed, .. } => {
            [(Outcome::ExitSuccess, success), (Outcome::ExitPending, pending), (Outcome::ExitFailure, failed)]
                .into_iter().filter_map(|(outcome, edge)| edge.as_ref().map(|edge| (outcome, edge))).collect()
        }
        TermKind::FailureDispatch { some, .. } => vec![(Outcome::FailureSome, some)],
        _ => vec![],
    }
}
