//! Declarative contracts shared by construction, analysis and verification.
//! These records describe obligations; they neither prove them nor run cleanup.
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Consume on a language argument grants access to its offered fresh part;
/// its reference remains copyable. On service values it consumes the SSA token.
pub enum OperandMode { Borrow, Consume, Observe, DestroyBorrow, Forward }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandConvention { Borrow, Consume }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperandUse { pub value: ValueId, pub mode: OperandMode }
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OperandRef { Value(ValueId), Selection(SelectionId), Dependency(DepExprId), Key(KeyExprId) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferencedOperand { pub reference: OperandRef, pub mode: OperandMode }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueFacts {
    Unelaborated,
    Plain { certificate: PlainCertificate },
    Managed { deps: DepExprId },
    Service,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlainCertificate { Scalar, ClosedGraph { ty: TypeId }, Descriptor(ValueId) }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstructionFacts {
    pub result_deps: Vec<(u16, DepExprId)>,
    pub retention: Vec<SelectionId>,
    pub provenance: Vec<ProvenanceFact>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvenanceFact { pub obligations: SelectionId, pub state: AdmissionProvenance }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionProvenance {
    UnboundResult, Bound, StagedOrCarrierOwned, CacheOwned,
    ExtractedAtExistingBoundary, TemporaryAwaitResult,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceValue { pub value: ValueId, pub packet: ValueId }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyExpr {
    Created { site: CreationSiteId, instance: ValueId },
    ServiceOf { value: ValueId, kind: ServiceKeyKind },
    Runtime(ValueId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceKeyKind { Cache, Frame, Channel, Carrier, Pin, Group }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DepExpr {
    Empty, Key(KeyExprId), Union(Vec<DepExprId>), FromSsa(ValueId),
    Retention { value: ValueId, selection: SelectionId },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionExpr {
    Value(ValueId), Projection(Projection), OwnerGraph(ValueId), ViewOwner(ValueId),
    /// Restrict to obligations with UnboundResult or TemporaryAwaitResult
    /// provenance; this never creates a new admission right.
    Admissible(SelectionId), Union(Vec<SelectionId>), Descriptor { descriptor: ValueId, value: ValueId },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projection {
    Field { value: ValueId, field: FieldRef },
    Index { value: ValueId, index: ValueId },
    Slice { value: ValueId, start: ValueId, end: ValueId },
    Map { value: ValueId, keys: ValueId },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldRef { pub declaration: DeclId, pub index: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boundary { pub scope: ScopeId, pub kind: BoundaryKind }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryKind { Lexical, Call, Loop, AbortBeforeDestroy, AbortAfterPrefix }

/// Builders resolve these formulas against input types and declarations;
/// #564 independently re-checks their semantic validity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultType {
    Exact(TypeId), ExactEnum(DeclId), Scalar(ScalarType), Service(ServiceType), SameAs(ValueId),
    Element(ValueId), MapKey(ValueId), MapValue(ValueId), Field(FieldRef),
    ArrayOf(Box<ResultType>), Object { decl: DeclId, args: Vec<TypeId> },
    FutureOf(Option<TypeId>), Awaited(ValueId), SuccessorOf(ValueId),
    BuilderOf(TypeId), MapBuilderOf(TypeId, TypeId), Finished(ValueId),
    Frame(FuncId), CellOf(TypeId), CellValue(ValueId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarType { Int, Bool, Byte, Float, Rat, Char, Str, Range, Any, Descriptor }
/// Names for fixed slots; indexed slots print as `$slotN` in the future codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotName {
    Value, Packet, Deps, Rest, Selected, ExternalDeps, State, From, To,
    Failure, Closed, Pending, Buffer, Unique, Receipt, Task, Pin, Snapshot, Slot(u16),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultSlot { pub name: SlotName, pub ty: ResultType }

/// Exact retention relations. Projected children do not retain their container;
/// owner-dependent views must additionally name ViewOwner or Descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Retention {
    Global(DeclId), Value(ValueId), Field { base: ValueId, field: FieldRef },
    Index { base: ValueId, index: ValueId },
    Slice { base: ValueId, start: ValueId, end: ValueId },
    MapEntry { map: ValueId, key: ValueId },
    MapPosition { map: ValueId, index: ValueId, key: bool },
    Keys(ValueId), Values(ValueId),
    ArrayReplace { array: ValueId, index: ValueId, value: ValueId },
    ArrayRemove { array: ValueId, index: ValueId },
    Difference { base: ValueId, removed: Vec<ValueId> },
    MapMerge(Vec<ValueId>), Union(Vec<ValueId>),
    ObjectFields { object: ValueId, updates: Vec<FieldValue> },
    Entries(Vec<(ValueId, ValueId)>),
    OwnerGraph(ValueId), ViewOwner(ValueId), Descriptor { descriptor: ValueId, value: ValueId },
    Cache(ValueId), TakenOutcome { future: ValueId, boundary: TakeBoundary },
    AwaitFailure { future: ValueId, take: Option<TakeBoundary> }, CarrierGraph(Vec<ValueId>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionRule { pub result: SlotName, pub selection: Retention }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvenanceTransition { Preserve, Set(AdmissionProvenance) }
impl ResultSlot {
    pub fn new(name: SlotName, ty: ResultType) -> Self { Self { name, ty } }
    pub fn service(name: SlotName, ty: ServiceType) -> Self { Self::new(name, ResultType::Service(ty)) }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutcomeContract {
    pub results: Vec<ResultSlot>,
    pub transfers: Vec<TransferRule>,
    pub postconditions: Vec<Postcondition>,
    pub retention: Vec<RetentionRule>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferRule {
    Preserve { input: ValueId, output: SlotName },
    Move { inputs: Vec<ValueId>, outputs: Vec<SlotName> },
    Partition { input: ValueId, selection: SelectionId, rest: SlotName, selected: SlotName },
    Admit { state: ValueId, packet: ValueId, output: SlotName, provenance: ProvenanceTransition },
    ScopeTransfer { states: TransferStates, selection: SelectionId },
    Settle { inputs: Vec<ValueId> },
    Close { input: ValueId },
    Protocol { protocol: ProtocolId, transition: ProtocolTransition, inputs: Vec<ValueId>, outputs: Vec<SlotName> },
    Publish { input: ValueId, holder: HolderKind, provenance: AdmissionProvenance },
    Extract { future: ValueId, capability: ValueId, boundary: TakeBoundary },
    Reserve { packet: ValueId, boundary: BoundaryId },
    /// Successful close forwards the reserve; exceptional close settles it in full.
    CloseReserve { scope: ValueId, reserve: ValueId, output: Option<SlotName> },
    ExtendReservation { pending: ValueId, packet: ValueId },
    ReplaceReservation { pending: ValueId, packet: Option<ValueId> },
    AbortReservation { pending: ValueId, cleanup: ValueId },
    FrameSave { frame: ValueId, values: Vec<ValueId> },
    FrameResume { frame: ValueId, slots: Vec<u32> },
    CompleteIdentity { identity: ValueId, state: ValueId, output: SlotName },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Postcondition {
    OwnerServiceDuty { value: SlotName, owner: ValueId, kind: ServiceKeyKind },
    Closed(ValueId), CacheConsumed(ValueId), AdmissionStaysCaller,
    PreserveFullDependencies, TransferableScopeDomainOnly,
    FreshAdmission(AdmissionProvenance), EpochOnRealTransfer,
    ProvesUniqueCache(UniqueCacheProof), WorkerSerializationOnly, CarrierGraph(ValueId),
    CleanupCompleteBeforeCommit, CancellationDeliveredBeforeCommit, ResultRelation(ResultRelation),
    NoPublication, CalleeInputsSettled, UniqueCacheRecheckAfterCoro,
    DestroyingBeforeCall, DeadAfterCall(ValueId), ContinueCleanup, Shielded,
    CleanupOwnsReserve, FirstFailureAbortsWholeReserveIntoSuffix, FailurePriorityPreserved,
    CleanupDrained, FailedExitHasNoPending,
    ServiceSettleBeforeSerialPlan, SerialOrder, NoFreshAdmission,
    ReceiptDeterminesHolder, TaskDeterminesHolder,
    PreserveProvenance, CacheConsumedIfSettled(ValueId), NonLinearSlotOnly,
    Dynamic { target: ScopeId, reason: DynamicReason, protocol: ProtocolId },
    LexicalExit { kind: ExitKind, boundary: BoundaryId, selection: SelectionId },
    CacheRehome(ScopeId), ConstructorFieldWrite(FieldRef), OnceValuePlain,
    IncompleteIdentity, CompleteIdentity(ValueId), DischargeMustUse(ValueId), CreatesMustUse,
    NoActiveFailureReplacement, FullCarrierGraphOwner, NativeTransferableOnly,
    ProtocolState { holder: HolderKind, state: HolderState },
    ProtocolAlternatives { protocol: ProtocolId, outcome: Outcome, states: Vec<ProtocolPoststate> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind { Thrown, Program, Panic, Native, Cancelled }
pub type FailureKindSet = Vec<FailureKind>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeEffectEnvelope {
    pub failures: FailureKindSet,
    pub suspends: bool,
    pub cancel: bool,
}
impl ScopeEffectEnvelope {
    /// Conservative envelope for producers before cleanup-effect inference narrows the site.
    pub fn conservative() -> Self {
        Self {
            failures: vec![FailureKind::Thrown, FailureKind::Program, FailureKind::Panic, FailureKind::Native],
            suspends: true,
            cancel: false,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    pub failures: Vec<FailureKind>,
    pub suspends: bool,
    pub allocates: bool,
    pub publishes: bool,
    pub cleanup: bool,
    pub mutates: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootKind { Operands, ActiveSelf, CleanupRemainder, PendingExit, CarrierGraph, Pins, FrameSlots, Cache, ServiceSchedule }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootContract { pub roots: Vec<RootKind>, pub through_suspend: bool }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CancellationContract {
    /// Cancelled is delivered on the unwind edge, with these exact slots.
    pub outcome: OutcomeContract,
    pub delivery: CancellationDelivery,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancellationDelivery { Unwind, AfterShieldedCleanup }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpContract {
    pub operands: Vec<ReferencedOperand>,
    pub normal: OutcomeContract,
    pub unwind: Option<OutcomeContract>,
    pub cancellation: Option<CancellationContract>,
    pub effects: Effects,
    pub roots: RootContract,
    pub entry_kind: EntryKind,
}
impl OpContract {
    pub fn outcome(&self, outcome: Outcome) -> Option<&OutcomeContract> {
        match outcome {
            Outcome::Normal => Some(&self.normal), Outcome::Unwind => self.unwind.as_ref(),
            Outcome::Cancel => self.cancellation.as_ref().map(|cancel| &cancel.outcome),
            Outcome::DeferContinue | Outcome::DeferReplace(_) | Outcome::ExitSuccess | Outcome::ExitPending
            | Outcome::ExitFailure | Outcome::FailureSome | Outcome::CleanupSelected | Outcome::CleanupDone => None,
        }
    }
    pub fn edge_result(&self, result: EdgeResult) -> Option<&ResultSlot> {
        self.outcome(result.outcome)?.results.get(usize::from(result.slot))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind { CallBoundary, ObservationAdmissionStaysCaller, Internal }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshConvention { Borrow, Take }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterEffect {
    pub convention: FreshConvention,
    pub retention: Option<SummarySelection>,
    pub deps: Vec<u16>,
    pub destination: TransferDestination,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummarySelection { Parameter(u16), Field { parameter: u16, field: FieldRef },
    Path { parameter: u16, fields: Vec<FieldRef> }, OwnerGraph(u16), Descriptor(u16), Union(Vec<SummarySelection>) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferDestination { None, Result, Scope(ScopeId), Protocol(ProtocolId), Cache, FailureContext }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRelation {
    pub fresh: Vec<CreationSiteId>,
    pub aliases: Vec<SummarySelection>,
    pub external_deps: Vec<u16>,
    pub provenance: AdmissionProvenance,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub entry_kind: EntryKind,
    pub parameters: Vec<ParameterEffect>,
    pub results: Vec<ResultRelation>,
    pub contract: CallContract,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallContract {
    pub results: Vec<TypeId>,
    pub failures: Vec<FailureKind>,
    pub suspends: bool,
    pub async_destroy: bool,
    pub publication: Publication,
    pub protocol: Option<ProtocolId>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publication { Atomic, ChannelReceipt, TaskState }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HolderKind { Packet, Scope, Frame, Cache, Channel, Carrier, PendingExit, Pin, Group, Diagnostic, Receipt }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HolderState { Open, Unaccepted, Accepted, Delivered, Running, Settled, Consumed, Released, Closed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolTransition { Attach, Transfer, Rehome, Release, Publish, Settle, Save, Resume, Cancel, Pin, Unpin, Dynamic }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolCheck { LiveDependencies, Snapshot, Owner, Transferable, UniqueCache, NoReentry, SufficientBoundary }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolPoststate { pub holder: HolderKind, pub state: HolderState }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolRule {
    pub transition: ProtocolTransition,
    pub from: HolderKind,
    pub to: HolderKind,
    pub checks: Vec<ProtocolCheck>,
    pub normal: ProtocolPoststate,
    pub unwind: Option<ProtocolPoststate>,
    pub cancel: Option<ProtocolPoststate>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtocolKind { Storage, Callable, Atomic, Cell, Channel, Group, Any, Closure, Services }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolDecl {
    pub kind: ProtocolKind,
    pub holders: Vec<HolderKind>,
    pub rules: Vec<ProtocolRule>,
    pub roots: Vec<RootKind>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptorContract {
    pub managed: DescriptorManaged,
    pub retention: SummarySelection,
    pub cleanup: FuncId,
    pub async_destroy: bool,
    pub summary: SummaryId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorManaged { Plain, Managed, Runtime }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BufferAction { Update, Finish, Abandon }
#[derive(Clone, Debug)]
pub(crate) struct Spec { pub name: &'static str, pub operands: Vec<ReferencedOperand>, pub rule: Rule }
#[derive(Clone, Debug)]
pub(crate) enum Rule {
    Data { ty: ResultType, trap: bool }, Intent, Check,
    Convert { from: NumericType, to: NumericType },
    Retained { rule: Box<Rule>, selection: Retention },
    Restricted { rule: Box<Rule>, requirement: Postcondition },
    Construct { ty: ResultType, packet: Option<ValueId>, checked: bool, identity: bool },
    ObjectInit { object: ValueId, field: FieldRef, completion: Option<ConstructionCompletion> },
    BufferNew(ResultType), Buffer { buffer: ValueId, packet: Option<ValueId>, action: BufferAction },
    Empty, Move { packets: Vec<ValueId> },
    Admit { state: ValueId, packet: ValueId, provenance: ProvenanceTransition },
    Stage(ElaboratedStage),
    Split { input: ValueId, selection: SelectionId, scope: bool, checked: bool },
    ScopeBegin(ScopeId), ScopeTransfer { states: TransferStates, selection: SelectionId },
    Dynamic { state: ValueId, packet: ValueId, target: ScopeId, reason: DynamicReason, protocol: ProtocolId },
    Close { input: ValueId, failure: Option<ValueId>, reserve: Option<ValueId>, effects: ScopeEffectEnvelope },
    Call(CallSite), Resume { frame: ValueId, call: CallSite },
    Await { future: ValueId, result: Option<TypeId>, take: Option<(ValueId, TakeBoundary)>, shielded: bool },
    UniqueProof(UniqueCacheProof),
    Protocol { state: ValueId, owner: Option<ValueId>, packet: Option<ValueId>, protocol: ProtocolId, transition: ProtocolTransition },
    FutureNew { environment: FutureEnvironment, packet: ValueId, owner: ValueId, result: Option<TypeId> },
    Publish { task: ValueId, cache: ValueId, packet: ValueId, protocol: ProtocolId },
    TaskSettle { task: ValueId, protocol: ProtocolId },
    FrameSave { frame: ValueId, slots: Vec<FrameSlot> },
    FrameResume { frame: ValueId, slots: Vec<SavedSlot> },
    FrameCancel { frame: ValueId, cleanup: ValueId, protocol: ProtocolId },
    Pin { owner: ValueId, protocol: ProtocolId }, Worker(Option<TypeId>),
    CarrierAttach { failure: ValueId, packet: ValueId, owner: ValueId, protocol: ProtocolId },
    CarrierTransfer { state: ValueId, target: CarrierDestination, protocol: ProtocolId },
    Prepare(PreparedExit), Replace { pending: ValueId, replacement: PreparedExit },
    ExtendReservation { pending: ValueId, packet: ValueId },
    Commit { pending: ValueId, result: Option<TypeId> }, Abort { pending: ValueId, cleanup: ValueId },
    CleanupBegin { state: ValueId, pending: Option<ValueId>, failure: Option<ValueId>, effects: ScopeEffectEnvelope },
    CleanupFailure { cleanup: ValueId },
    CleanupStep { cleanup: ValueId, finishing: bool }, CleanupNext(ValueId),
    CleanupSettle { cleanup: ValueId, holder: ValueId, protocol: ProtocolId },
    Destroy { cleanup: ValueId, identity: ValueId },
}

impl Rule {
    pub(crate) fn retains(self, selection: Retention) -> Self { Self::Retained { rule: Box::new(self), selection } }
    pub(crate) fn requires(self, requirement: Postcondition) -> Self { Self::Restricted { rule: Box::new(self), requirement } }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    NotDeferEntry,
    UnknownDeferLabel,
    InvalidId(ArenaError),
    MissingTransition { protocol: ProtocolId, transition: ProtocolTransition },
    CallArity { expected: usize, actual: usize },
    CallConvention { argument: usize },
    MissingSummary,
    ObservationPacket,
    PublicationNeedsProtocol,
    TooManyResults,
    InvalidConversion { from: NumericType, to: NumericType },
}
impl From<ArenaError> for ContractError { fn from(error: ArenaError) -> Self { Self::InvalidId(error) } }
impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for ContractError {}

/// Builtin contracts are fixed by the opcode. Only explicit call and protocol
/// references consult declarations; an absent declaration is always an error.
#[derive(Clone, Copy, Debug)]
pub struct ContractRegistry<'a> { pub module: &'a Module }
impl<'a> ContractRegistry<'a> {
    pub fn new(module: &'a Module) -> Self { Self { module } }
    pub fn lookup(&self, op: &Operation) -> Result<OpContract, ContractError> {
        self.lookup_mode(op, true, None)
    }
    /// Outcome shapes for text edge slots, without semantic call/protocol checks.
    pub(crate) fn text_schema(&self, op: &Operation, body: &Body) -> Result<OpContract, ContractError> {
        self.lookup_mode(op, false, Some(body))
    }
    /// High construction has typed call results before summary inference. This
    /// supplies only a conservative shape, never ownership postconditions.
    pub(crate) fn construction_schema(&self, op: &Operation, body: &Body) -> Result<OpContract, ContractError> {
        if self.module.profile == Profile::High {
            self.lookup_mode(op, true, Some(body))
        } else { self.lookup(op) }
    }
    /// Site metadata is available to verification before ownership summary inference.
    pub fn high_site_schema(
        &self, op: &Operation, body: &Body, site: HighOperationSite,
    ) -> Result<OpContract, ContractError> {
        let mut contract = self.construction_schema(op, body)?;
        if self.module.profile == Profile::High
            && let Some(record) = body.high_effects.iter().find(|record| record.site == site) {
            contract.effects.failures = record.effects.failures.clone();
            contract.effects.suspends = record.effects.may_suspend;
            contract.roots.through_suspend = record.effects.may_suspend;
            if record.effects.failures.is_empty() {
                contract.unwind = None;
            }
            if !record.effects.failures.contains(&FailureKind::Cancelled) {
                contract.cancellation = None;
            }
        }
        Ok(contract)
    }
    fn lookup_mode(&self, op: &Operation, validate: bool, body: Option<&Body>) -> Result<OpContract, ContractError> {
        let spec = op.spec();
        let mut contract = OpContract {
            operands: spec.operands,
            normal: outcome(vec![]), unwind: None, cancellation: None,
            effects: Effects::default(),
            roots: RootContract { roots: vec![RootKind::Operands], through_suspend: false },
            entry_kind: EntryKind::Internal,
        };
        if (!validate || self.module.profile == Profile::High && body.is_some()) && let Operation::Call(call_op) = op {
            let call = match call_op {
                CallOp::Direct { call, .. } | CallOp::Native { call, .. }
                | CallOp::Intrinsic { call, .. } | CallOp::Resume { call, .. }
                | CallOp::Role { call, .. } | CallOp::Closure { call, .. } | CallOp::Cap { call, .. } => call,
            };
            if call.summary.is_none() {
                let results = self.unelaborated_results(call_op, body)?;
                for (index, &ty) in results.iter().enumerate() {
                    contract.normal.results.push(ResultSlot::new(if results.len() == 1 { SlotName::Value } else { SlotName::Slot(u16::try_from(index).map_err(|_| ContractError::TooManyResults)?) }, ResultType::Exact(ty)));
                }
                if call.packet.is_some() { contract.normal.results.extend([ResultSlot::service(SlotName::Packet, ServiceType::Own), ResultSlot::service(SlotName::Deps, ServiceType::Deps)]); }
                contract.unwind = Some(outcome(vec![ResultSlot::service(SlotName::Failure, ServiceType::Failure)]));
                contract.effects.failures = vec![FailureKind::Thrown, FailureKind::Program, FailureKind::Panic, FailureKind::Native, FailureKind::Cancelled];
                contract.effects.suspends = true;
                contract.roots.through_suspend = true;
                contract.cancellation = Some(CancellationContract { outcome: contract.unwind.clone().unwrap(), delivery: CancellationDelivery::Unwind });
                return Ok(contract);
            }
        }
        self.apply(spec.rule, &mut contract, validate)?;
        contract.roots.through_suspend = contract.effects.suspends;
        Ok(contract)
    }
    fn unelaborated_results(&self, op: &CallOp, body: Option<&Body>) -> Result<Vec<TypeId>, ContractError> {
        let signature = |function| Ok(self.module.functions.get(function)?.signature.results.clone());
        match op {
            CallOp::Direct { function, .. } | CallOp::Native { function, .. }
            | CallOp::Intrinsic { function, .. } | CallOp::Resume { function, .. } => signature(*function),
            CallOp::Closure { callee, .. } => {
                let body = body.ok_or(ContractError::MissingSummary)?;
                match self.module.types.get(body.values.get(callee.value)?.ty)? {
                    Type::Fn(ty) => Ok(ty.result.iter().copied().collect()),
                    _ => Err(ContractError::MissingSummary),
                }
            }
            CallOp::Role { role, slot, .. } => match &self.module.declarations.get(*role)?.kind {
                DeclarationKind::Role { methods, .. } => methods.get(*slot as usize).map(|m| m.signature.results.clone()).ok_or(ContractError::MissingSummary),
                _ => Err(ContractError::MissingSummary),
            },
            CallOp::Cap { field, .. } => match &self.module.declarations.get(field.declaration)?.kind {
                DeclarationKind::Cap { fields, .. } => signature(fields.get(field.index as usize).ok_or(ContractError::MissingSummary)?.function),
                _ => Err(ContractError::MissingSummary),
            },
        }
    }
    fn protocol(&self, id: ProtocolId, transition: ProtocolTransition, c: &mut OpContract, validate: bool) -> Result<(), ContractError> {
        let decl = self.module.protocols.get(id)?;
        if !validate { return Ok(()); }
        let rules: Vec<_> = decl.rules.iter().filter(|rule| rule.transition == transition).collect();
        if rules.is_empty() { return Err(ContractError::MissingTransition { protocol: id, transition }); }
        c.roots.roots.extend(decl.roots.iter().copied());
        c.normal.transfers.push(TransferRule::Protocol { protocol: id, transition,
            inputs: c.operands.iter().filter_map(|operand| match operand { ReferencedOperand { reference: OperandRef::Value(value), mode: OperandMode::Consume } => Some(*value), _ => None }).collect(),
            outputs: c.normal.results.iter().map(|slot| slot.name).collect(),
        });
        c.normal.postconditions.push(Postcondition::EpochOnRealTransfer);
        protocol_states(id, Outcome::Normal, rules.iter().map(|rule| rule.normal).collect(), &mut c.normal);
        // The protocol's state alternatives select the holder at unwind/cancel;
        // they do not promise that the original packet comes back.
        for (outcome_kind, target) in [(Outcome::Unwind, c.unwind.as_mut()),
            (Outcome::Cancel, c.cancellation.as_mut().map(|cancel| &mut cancel.outcome))] {
            if let Some(out) = target {
                if out.transfers.is_empty() {
                    out.transfers.push(TransferRule::Protocol { protocol: id, transition,
                        inputs: c.operands.iter().filter_map(|operand| match operand { ReferencedOperand { reference: OperandRef::Value(value), mode: OperandMode::Consume } => Some(*value), _ => None }).collect(),
                        outputs: out.results.iter().map(|slot| slot.name).collect(),
                    });
                }
                protocol_states(id, outcome_kind, rules.iter().filter_map(|rule| match outcome_kind { Outcome::Cancel => rule.cancel, _ => rule.unwind }).collect(), out);
            }
        }
        Ok(())
    }
    fn apply(&self, rule: Rule, c: &mut OpContract, validate: bool) -> Result<(), ContractError> {
        use SlotName as N;
        use ServiceType as S;
        match rule {
            Rule::Data { ty, trap } => {
                c.normal.results.push(ResultSlot::new(N::Value, ty));
                observation(c);
                if trap { fails(c, vec![], false); }
            }
            Rule::Convert { from, to } => {
                use NumericType as T;
                let trap = match (from, to) {
                    (T::Int, T::Float | T::Byte) | (T::Float, T::Int | T::Byte) | (T::Rat, T::Float | T::Byte) => true,
                    (T::Byte, T::Int | T::Float | T::Rat) | (T::Int | T::Float, T::Rat) | (T::Rat, T::Int) |
                    (T::Int | T::Byte | T::Float | T::Rat | T::Bool, T::Str) => false,
                    _ => return Err(ContractError::InvalidConversion { from, to }),
                };
                self.apply(Rule::Data { ty: to.result_type(), trap }, c, validate)?;
            }
            Rule::Retained { rule, selection } => {
                self.apply(*rule, c, validate)?;
                c.normal.retention.push(RetentionRule { result: SlotName::Value, selection });
            }
            Rule::Restricted { rule, requirement } => {
                self.apply(*rule, c, validate)?; c.normal.postconditions.push(requirement);
            }
            Rule::Intent => { c.effects.mutates = true; }
            Rule::Check => { observation(c); fails(c, vec![], false); }
            Rule::Construct { ty, packet, checked, identity } => {
                c.effects.allocates = true;
                c.normal.results.push(ResultSlot::new(N::Value, ty));
                if let Some(packet) = packet {
                    c.normal.results.push(ResultSlot::service(N::Packet, S::Own));
                    c.normal.transfers.push(TransferRule::Move { inputs: vec![packet], outputs: vec![N::Packet] });
                }
                c.normal.postconditions.push(Postcondition::PreserveFullDependencies);
                if identity { c.normal.postconditions.push(Postcondition::FreshAdmission(AdmissionProvenance::UnboundResult)); }
                if checked { fails(c, preserve(packet, N::Packet), false); }
            }
            Rule::ObjectInit { object, field, completion } => {
                c.effects.mutates = true;
                c.normal.postconditions.push(Postcondition::ConstructorFieldWrite(field));
                if let Some(completion) = completion {
                    c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(completion.state)));
                    c.normal.transfers.push(TransferRule::CompleteIdentity { identity: object, state: completion.state, output: N::State });
                    c.normal.postconditions.extend([Postcondition::CompleteIdentity(object), Postcondition::FreshAdmission(AdmissionProvenance::UnboundResult)]);
                }
            }
            Rule::BufferNew(ty) => {
                c.normal.results.push(ResultSlot::new(N::Buffer, ty)); c.effects.allocates = true;
            }
            Rule::Buffer { buffer, packet, action } => {
                c.effects.allocates = !matches!(action, BufferAction::Abandon);
                let name = if matches!(action, BufferAction::Update) { N::Buffer } else { N::Value };
                if !matches!(action, BufferAction::Abandon) {
                    c.normal.results.push(ResultSlot::new(name, if matches!(action, BufferAction::Update) {
                        ResultType::SameAs(buffer)
                    } else { ResultType::Finished(buffer) }));
                }
                // The element journal is explicit even on abandon; its receiver
                // must admit the returned packet at the original owner boundary.
                if packet.is_some() { c.normal.results.push(ResultSlot::service(N::Packet, S::Own)); }
                c.normal.transfers.push(TransferRule::Move {
                    inputs: std::iter::once(buffer).chain(packet).collect(), outputs: c.normal.results.iter().map(|slot| slot.name).collect(),
                });
                c.normal.postconditions.push(Postcondition::PreserveFullDependencies);
                if !matches!(action, BufferAction::Abandon) {
                    let mut restored = preserve(Some(buffer), N::Buffer); restored.extend(preserve(packet, N::Packet));
                    fails(c, restored, false);
                }
            }
            Rule::Empty => { c.normal.results.push(ResultSlot::service(N::Packet, S::Own)); }
            Rule::Move { packets } => {
                c.normal.results.push(ResultSlot::service(N::Packet, S::Own));
                c.normal.transfers.push(TransferRule::Move { inputs: packets, outputs: vec![N::Packet] });
                c.normal.postconditions.push(Postcondition::PreserveFullDependencies);
            }
            Rule::Admit { state, packet, provenance } => {
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(state)));
                c.normal.transfers.push(TransferRule::Admit { state, packet, output: N::State, provenance });
                c.normal.postconditions.push(match provenance { ProvenanceTransition::Preserve => Postcondition::PreserveProvenance,
                    ProvenanceTransition::Set(state) => Postcondition::FreshAdmission(state) });
            }
            Rule::Stage(stage) => {
                state_results(stage.states, &mut c.normal);
                c.normal.transfers.push(TransferRule::Move { inputs: std::iter::once(stage.resource.packet).chain(stage.states.values()).collect(), outputs: c.normal.results.iter().map(|slot| slot.name).collect() });
                c.normal.postconditions.extend([Postcondition::FreshAdmission(AdmissionProvenance::StagedOrCarrierOwned), Postcondition::EpochOnRealTransfer]);
            }
            Rule::Split { input, selection, scope, checked } => {
                let rest = if scope { N::State } else { N::Rest };
                c.normal.results = vec![ResultSlot::new(rest, if scope { ResultType::SameAs(input) } else { ResultType::Service(S::Own) }),
                    ResultSlot::service(N::Selected, S::Own), ResultSlot::service(N::ExternalDeps, S::Deps)];
                c.normal.transfers.push(TransferRule::Partition { input, selection, rest, selected: N::Selected });
                c.normal.postconditions.push(Postcondition::PreserveFullDependencies);
                if scope { c.normal.postconditions.push(Postcondition::TransferableScopeDomainOnly); }
                if checked { fails(c, preserve(Some(input), if scope { N::State } else { N::Packet }), false); }
            }
            Rule::ScopeBegin(scope) => { c.normal.results.push(ResultSlot::service(N::State, S::Scope(scope))); }
            Rule::ScopeTransfer { states, selection } => {
                state_results(states, &mut c.normal);
                c.normal.transfers.push(TransferRule::ScopeTransfer { states, selection });
                c.normal.postconditions.extend([Postcondition::TransferableScopeDomainOnly, Postcondition::EpochOnRealTransfer, Postcondition::PreserveFullDependencies]);
                let restored = match states { TransferStates::Same(state) => preserve(Some(state), N::State),
                    TransferStates::Between { from, to } => [preserve(Some(from), N::From), preserve(Some(to), N::To)].concat() };
                fails(c, restored, false);
            }
            Rule::Dynamic { state, packet, target, reason, protocol } => {
                c.normal.postconditions.push(Postcondition::Dynamic { target, reason, protocol });
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(state)));
                c.normal.transfers.push(TransferRule::Admit { state, packet, output: N::State, provenance: ProvenanceTransition::Preserve });
                fails(c, [preserve(Some(state), N::State), preserve(Some(packet), N::Packet)].concat(), false);
                self.protocol(protocol, ProtocolTransition::Dynamic, c, validate)?;
            }
            Rule::Close { input, failure, reserve, effects } => {
                cleanup(c);
                c.effects.failures = effects.failures.clone();
                c.effects.suspends = effects.suspends;
                if effects.cancel {
                    c.effects.failures.push(FailureKind::Cancelled);
                }
                c.normal.transfers.push(TransferRule::Close { input });
                c.normal.postconditions.push(Postcondition::Closed(input));
                if reserve.is_some() {
                    c.roots.roots.push(RootKind::PendingExit);
                }
                if failure.is_some() {
                    c.normal.results.push(ResultSlot::service(N::Failure, S::Failure));
                    if let Some(reserve) = reserve {
                        c.normal.transfers.push(TransferRule::CloseReserve { scope: input, reserve, output: None });
                    }
                } else {
                    c.normal.results.push(ResultSlot::service(N::Closed, S::Closed));
                    let mut unwind = outcome(vec![
                        ResultSlot::service(N::Closed, S::Closed),
                        ResultSlot::service(N::Failure, S::Failure),
                    ]);
                    unwind.transfers.push(TransferRule::Close { input });
                    unwind.postconditions = c.normal.postconditions.clone();
                    if let Some(reserve) = reserve {
                        c.normal.results.push(ResultSlot::service(N::Pending, S::Pending));
                        c.normal.transfers.push(TransferRule::CloseReserve {
                            scope: input, reserve, output: Some(N::Pending),
                        });
                        unwind.transfers.push(TransferRule::CloseReserve { scope: input, reserve, output: None });
                    }
                    if !effects.failures.is_empty() || effects.cancel {
                        c.unwind = Some(unwind.clone());
                    }
                    if effects.cancel {
                        c.cancellation = Some(CancellationContract {
                            outcome: unwind, delivery: CancellationDelivery::AfterShieldedCleanup,
                        });
                    }
                }
            }
            Rule::Call(call) => { self.call(&call, c, validate)?; }
            Rule::Resume { frame, call } => {
                self.call(&call, c, validate)?;
                c.normal.transfers.push(TransferRule::Settle { inputs: vec![frame] });
                if let Some(unwind) = &mut c.unwind { unwind.transfers.push(TransferRule::Settle { inputs: vec![frame] }); }
                if let Some(cancel) = &mut c.cancellation { cancel.outcome.transfers.push(TransferRule::Settle { inputs: vec![frame] }); }
                c.roots.roots.push(RootKind::FrameSlots);
            }
            Rule::Await { future, result, take, shielded } => {
                c.effects.suspends = true;
                c.effects.failures = all_failures();
                c.roots.roots.extend([RootKind::Cache, RootKind::FrameSlots, RootKind::CarrierGraph, RootKind::Pins]);
                if let Some(ty) = result { c.normal.results.push(ResultSlot::new(N::Value, ResultType::Exact(ty))); }
                if let Some((unique, boundary)) = take {
                    c.normal.results.push(ResultSlot::service(N::Packet, S::Own));
                    c.normal.transfers.push(TransferRule::Extract { future, capability: unique, boundary });
                    c.normal.postconditions.extend([Postcondition::CacheConsumed(future), Postcondition::FreshAdmission(match boundary {
                        TakeBoundary::Existing { .. } => AdmissionProvenance::ExtractedAtExistingBoundary,
                        TakeBoundary::Temporary { .. } => AdmissionProvenance::TemporaryAwaitResult,
                    })]);
                } else {
                    c.normal.postconditions.extend([Postcondition::NoFreshAdmission, Postcondition::PreserveFullDependencies]);
                }
                if self.module.profile != Profile::High {
                    c.normal.results.push(ResultSlot::service(N::Deps, S::Deps));
                }
                if result.is_some() { c.normal.retention.push(RetentionRule { result: N::Value, selection: match take {
                    Some((_, boundary)) => Retention::TakenOutcome { future, boundary }, None => Retention::Cache(future),
                } }); }
                let mut unwind = outcome(vec![ResultSlot::service(N::Failure, S::Failure)]);
                unwind.postconditions.push(Postcondition::NoPublication);
                if self.module.profile != Profile::High {
                    unwind.results.extend([
                        ResultSlot::service(N::Task, S::TaskState),
                        ResultSlot::service(N::Receipt, S::Receipt),
                    ]);
                    unwind.postconditions.extend([
                        Postcondition::TaskDeterminesHolder, Postcondition::ReceiptDeterminesHolder,
                    ]);
                }
                unwind.retention.push(RetentionRule { result: N::Failure, selection: Retention::AwaitFailure { future, take: take.map(|(_, boundary)| boundary) } });
                if let Some((unique, _)) = take {
                    unwind.transfers.push(TransferRule::Settle { inputs: vec![unique] });
                    unwind.postconditions.push(Postcondition::CacheConsumedIfSettled(future));
                }
                c.unwind = Some(unwind.clone());
                if !shielded { c.cancellation = Some(CancellationContract { outcome: unwind, delivery: CancellationDelivery::Unwind }); }
                else { c.normal.postconditions.push(Postcondition::Shielded); c.effects.failures.retain(|kind| *kind != FailureKind::Cancelled); }
            }
            Rule::UniqueProof(proof) => {
                c.normal.results.push(ResultSlot::service(N::Unique, S::UniqueCache));
                c.normal.postconditions.extend([Postcondition::UniqueCacheRecheckAfterCoro, Postcondition::ProvesUniqueCache(proof)]);
            }
            Rule::Protocol { state, owner, packet, protocol, transition } => {
                let closing = matches!(transition, ProtocolTransition::Release | ProtocolTransition::Unpin);
                c.normal.results.push(if closing { ResultSlot::service(N::Closed, S::Closed) }
                    else { ResultSlot::new(N::State, ResultType::SameAs(state)) });
                if let Some(owner) = owner { c.normal.results.push(ResultSlot::new(N::To, ResultType::SameAs(owner))); }
                let mut restored = preserve(Some(state), N::State);
                restored.extend(preserve(owner, N::To)); restored.extend(preserve(packet, N::Packet));
                fails(c, restored, false);
                c.normal.postconditions.push(Postcondition::NoFreshAdmission);
                self.protocol(protocol, transition, c, validate)?;
            }
            Rule::FutureNew { environment, packet, owner, result } => {
                c.normal.results = vec![
                    ResultSlot::new(N::Value, ResultType::FutureOf(result)),
                    ResultSlot::new(N::To, ResultType::SameAs(owner)),
                ];
                let mut inputs = vec![packet, owner];
                let mut outputs = vec![N::To];
                if let FutureEnvironment::Frame(frame) = environment {
                    inputs.push(frame);
                    if self.module.profile != Profile::Own {
                        c.normal.results.insert(1, ResultSlot::service(N::State, S::CacheState));
                        outputs.insert(0, N::State);
                    }
                }
                c.normal.transfers.push(TransferRule::Move { inputs, outputs });
                c.normal.postconditions.push(Postcondition::OwnerServiceDuty {
                    value: N::Value, owner, kind: ServiceKeyKind::Cache,
                });
                c.roots.roots.push(RootKind::Cache);
                c.effects.allocates = true;
            }
            Rule::Publish { task, cache, packet, protocol } => {
                c.normal.results.push(ResultSlot::service(N::State, S::CacheState));
                c.normal.transfers.push(TransferRule::Publish { input: packet, holder: HolderKind::Cache, provenance: AdmissionProvenance::CacheOwned });
                c.normal.transfers.push(TransferRule::Settle { inputs: vec![task, cache] });
                c.normal.postconditions.push(Postcondition::Closed(task));
                c.effects.publishes = true;
                self.protocol(protocol, ProtocolTransition::Publish, c, validate)?;
            }
            Rule::TaskSettle { task, protocol } => {
                cleanup(c);
                c.normal.results = vec![ResultSlot::service(N::Task, S::TaskState), ResultSlot::service(N::Packet, S::Own), ResultSlot::service(N::Failure, S::FailureState)];
                c.normal.transfers.push(TransferRule::Settle { inputs: vec![task] });
                self.protocol(protocol, ProtocolTransition::Settle, c, validate)?;
            }
            Rule::FrameSave { frame, slots } => {
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(frame)));
                c.normal.transfers.push(TransferRule::FrameSave { frame, values: slots.iter().filter(|slot| slot.mode == OperandMode::Consume).map(|slot| slot.value).collect() });
                c.roots.roots.push(RootKind::FrameSlots);
            }
            Rule::FrameResume { frame, slots } => {
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(frame)));
                for (index, slot) in slots.iter().enumerate() {
                    c.normal.results.push(ResultSlot::new(N::Slot(u16::try_from(index).map_err(|_| ContractError::TooManyResults)?), ResultType::Exact(slot.ty)));
                }
                c.normal.transfers.push(TransferRule::FrameResume { frame, slots: slots.iter().map(|slot| slot.slot).collect() });
                c.roots.roots.push(RootKind::FrameSlots);
            }
            Rule::FrameCancel { frame, cleanup: plan, protocol } => {
                cleanup(c);
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(plan)));
                c.normal.transfers.push(TransferRule::Settle { inputs: vec![frame, plan] });
                self.protocol(protocol, ProtocolTransition::Cancel, c, validate)?;
            }
            Rule::Pin { owner, protocol } => {
                c.normal.results = vec![ResultSlot::service(N::Pin, S::Pin), ResultSlot::new(N::State, ResultType::SameAs(owner))];
                fails(c, preserve(Some(owner), N::State), false);
                self.protocol(protocol, ProtocolTransition::Pin, c, validate)?;
            }
            Rule::Worker(result) => {
                c.normal.results.push(ResultSlot::new(N::Value, ResultType::FutureOf(result)));
                fails(c, vec![], false); c.effects.allocates = true;
                c.normal.postconditions.push(Postcondition::WorkerSerializationOnly);
            }
            Rule::CarrierAttach { failure, packet, owner, protocol } => {
                c.normal.results = vec![ResultSlot::service(N::State, S::CarrierState), ResultSlot::new(N::To, ResultType::SameAs(owner))];
                c.normal.transfers.push(TransferRule::Publish { input: packet, holder: HolderKind::Carrier, provenance: AdmissionProvenance::StagedOrCarrierOwned });
                c.normal.postconditions.extend([Postcondition::CarrierGraph(failure), Postcondition::FullCarrierGraphOwner]);
                fails(c, [preserve(Some(packet), N::Packet), preserve(Some(owner), N::To)].concat(), false);
                self.protocol(protocol, ProtocolTransition::Attach, c, validate)?;
            }
            Rule::CarrierTransfer { state, target, protocol } => {
                c.normal.results.push(ResultSlot::service(N::State, S::CarrierState));
                let mut restored = preserve(Some(state), N::State);
                if let CarrierDestination::Cache(value) | CarrierDestination::Diagnostic(value) = target {
                    c.normal.results.push(ResultSlot::new(N::To, ResultType::SameAs(value)));
                    restored.extend(preserve(Some(value), N::To));
                }
                fails(c, restored, false);
                c.normal.postconditions.push(Postcondition::NoFreshAdmission);
                self.protocol(protocol, ProtocolTransition::Transfer, c, validate)?;
            }
            Rule::Prepare(exit) => {
                if self.module.profile != Profile::High {
                    c.normal.results.push(ResultSlot::service(N::State, S::Pending));
                }
                if let Some(packet) = exit.packet { c.normal.transfers.push(TransferRule::Reserve { packet, boundary: exit.boundary }); }
                c.normal.postconditions.push(Postcondition::LexicalExit { kind: exit.kind, boundary: exit.boundary, selection: exit.selection });
                c.roots.roots.push(RootKind::PendingExit);
            }
            Rule::Replace { pending, replacement } => {
                c.normal.results = vec![ResultSlot::service(N::State, S::Pending)];
                if replacement.packet.is_some() { c.normal.results.push(ResultSlot::service(N::Rest, S::Own)); }
                c.normal.transfers.push(TransferRule::ReplaceReservation { pending, packet: replacement.packet });
                c.normal.postconditions.push(Postcondition::NoActiveFailureReplacement);
                c.normal.postconditions.push(Postcondition::LexicalExit { kind: replacement.kind, boundary: replacement.boundary, selection: replacement.selection });
                c.normal.postconditions.push(Postcondition::PreserveFullDependencies);
                c.roots.roots.push(RootKind::PendingExit);
            }
            Rule::ExtendReservation { pending, packet } => {
                c.normal.results.push(ResultSlot::service(N::State, S::Pending));
                c.normal.transfers.push(TransferRule::ExtendReservation { pending, packet });
                c.normal.postconditions.push(Postcondition::PreserveFullDependencies);
                c.roots.roots.push(RootKind::PendingExit);
            }
            Rule::Commit { pending, result } => {
                if let Some(ty) = result { c.normal.results.push(ResultSlot::new(N::Value, ResultType::Exact(ty))); }
                c.normal.results.extend([ResultSlot::service(N::Packet, S::Own), ResultSlot::service(N::Deps, S::Deps)]);
                c.normal.transfers.push(TransferRule::Move { inputs: vec![pending], outputs: vec![N::Packet] });
                c.normal.postconditions.extend([Postcondition::CleanupCompleteBeforeCommit, Postcondition::CancellationDeliveredBeforeCommit, Postcondition::FreshAdmission(AdmissionProvenance::UnboundResult)]);
                c.effects.publishes = true; c.roots.roots.push(RootKind::PendingExit);
            }
            Rule::Abort { pending, cleanup: plan } => {
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(plan)));
                c.normal.transfers.push(TransferRule::AbortReservation { pending, cleanup: plan });
                c.normal.postconditions.push(Postcondition::SerialOrder);
                c.roots.roots.extend([RootKind::PendingExit, RootKind::CleanupRemainder]);
            }
            Rule::CleanupBegin { state, pending, failure, effects } => {
                cleanup(c);
                c.effects.failures = effects.failures;
                c.effects.suspends = effects.suspends;
                if pending.is_some() {
                    c.normal.postconditions.push(Postcondition::CleanupOwnsReserve);
                }
                if failure.is_some() {
                    c.normal.postconditions.push(Postcondition::FirstFailureAbortsWholeReserveIntoSuffix);
                }
                c.normal.postconditions.push(Postcondition::FailurePriorityPreserved);
                c.normal.results.push(ResultSlot::service(N::State, S::CleanupState));
                c.normal.transfers.push(TransferRule::Move { inputs: std::iter::once(state).chain(pending).collect(), outputs: vec![N::State] });
            }
            Rule::CleanupFailure { cleanup: plan } => {
                if self.module.profile == Profile::Low {
                    captured_cleanup(c);
                } else {
                    cleanup(c);
                }
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(plan)));
                c.normal.transfers.push(TransferRule::Move { inputs: vec![plan], outputs: vec![N::State] });
                c.normal.postconditions.extend([Postcondition::FirstFailureAbortsWholeReserveIntoSuffix,
                    Postcondition::FailurePriorityPreserved]);
            }
            Rule::CleanupStep { cleanup: plan, finishing } => {
                if self.module.profile == Profile::Low {
                    captured_cleanup(c);
                } else {
                    cleanup(c);
                }
                if finishing {
                    c.normal.results = vec![ResultSlot::service(N::Closed, S::Closed), ResultSlot::service(N::Failure, S::FailureState), ResultSlot::service(N::State, S::ExitState)];
                    c.normal.transfers.push(TransferRule::Close { input: plan });
                    c.normal.postconditions.extend([Postcondition::Closed(plan), Postcondition::CleanupDrained,
                        Postcondition::FailedExitHasNoPending]);
                } else {
                    c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(plan)));
                    c.normal.transfers.push(TransferRule::Move { inputs: vec![plan], outputs: vec![N::State] });
                }
            }
            Rule::CleanupNext(plan) => {
                cleanup(c);
                c.normal.results = vec![ResultSlot::new(N::State, ResultType::SameAs(plan)), ResultSlot::service(N::Selected, S::StableKey), ResultSlot::new(N::Value, ResultType::Scalar(ScalarType::Bool))];
                c.normal.transfers.push(TransferRule::Move { inputs: vec![plan], outputs: vec![N::State] });
            }
            Rule::CleanupSettle { cleanup: plan, holder, protocol } => {
                cleanup(c);
                c.normal.results.push(ResultSlot::new(N::State, ResultType::SameAs(plan)));
                c.normal.transfers.push(TransferRule::Settle { inputs: vec![holder, plan] });
                self.protocol(protocol, ProtocolTransition::Settle, c, validate)?;
            }
            Rule::Destroy { cleanup: plan, identity } => {
                if self.module.profile == Profile::Low {
                    captured_cleanup(c);
                } else {
                    cleanup(c);
                }
                c.normal.results = vec![ResultSlot::new(N::State, ResultType::SameAs(plan)), ResultSlot::service(N::Failure, S::FailureState)];
                c.normal.transfers.push(TransferRule::Move { inputs: vec![plan], outputs: vec![N::State] });
                c.normal.postconditions.extend([Postcondition::DestroyingBeforeCall, Postcondition::DeadAfterCall(identity), Postcondition::ContinueCleanup]);
            }
        }
        Ok(())
    }

    /// Per-edge schemas for the tagged defer ABI. Replacement indices are declaration ordered.
    pub fn defer_results(&self, function: FuncId, outcome: Outcome) -> Result<Vec<ResultSlot>, ContractError> {
        let declaration = self.module.functions.get(function)?;
        let labels = declaration.attrs.iter().find_map(|attr| {
            if let FunctionAttr::DeferEntry { replacements } = attr { Some(replacements) } else { None }
        }).ok_or(ContractError::NotDeferEntry)?;
        match outcome {
            Outcome::DeferContinue => Ok(vec![]),
            Outcome::DeferReplace(index) => {
                let label = labels.get(usize::from(index)).ok_or(ContractError::UnknownDeferLabel)?;
                let mut results = vec![];
                if let Some(ty) = label.result {
                    results.push(ResultSlot::new(SlotName::Value, ResultType::Exact(ty)));
                }
                results.push(ResultSlot::service(SlotName::Packet, ServiceType::Own));
                results.push(ResultSlot::service(SlotName::Deps, ServiceType::Deps));
                Ok(results)
            }
            Outcome::Unwind => Ok(vec![ResultSlot::service(SlotName::Failure, ServiceType::Failure)]),
            _ => Err(ContractError::UnknownDeferLabel),
        }
    }

    fn call(&self, call: &CallSite, c: &mut OpContract, validate: bool) -> Result<(), ContractError> {
        use SlotName as N;
        let summary = self.module.summaries.get(call.summary.ok_or(ContractError::MissingSummary)?)?;
        if validate && call.args.len() != summary.parameters.len() {
            return Err(ContractError::CallArity { expected: summary.parameters.len(), actual: call.args.len() });
        }
        for (index, (arg, param)) in call.args.iter().zip(&summary.parameters).enumerate() {
            if validate && !matches!((arg.convention, param.convention), (OperandConvention::Borrow, FreshConvention::Borrow) | (OperandConvention::Consume, FreshConvention::Take)) {
                return Err(ContractError::CallConvention { argument: index });
            }
        }
        c.entry_kind = summary.entry_kind;
        if validate && summary.entry_kind == EntryKind::ObservationAdmissionStaysCaller && call.packet.is_some() { return Err(ContractError::ObservationPacket); }
        for (index, &ty) in summary.contract.results.iter().enumerate() {
            c.normal.results.push(ResultSlot::new(if summary.contract.results.len() == 1 { N::Value } else { N::Slot(u16::try_from(index).map_err(|_| ContractError::TooManyResults)?) }, ResultType::Exact(ty)));
        }
        if let Some(packet) = call.packet {
            c.normal.results.extend([ResultSlot::service(N::Packet, ServiceType::Own), ResultSlot::service(N::Deps, ServiceType::Deps)]);
            c.normal.transfers.push(TransferRule::Move { inputs: vec![packet], outputs: vec![N::Packet] });
        }
        c.effects.failures = summary.contract.failures.clone();
        c.effects.suspends = summary.contract.suspends || summary.contract.async_destroy;
        c.roots.roots.extend([RootKind::CarrierGraph, RootKind::PendingExit]);
        if c.effects.suspends { c.roots.roots.extend([RootKind::FrameSlots, RootKind::Pins, RootKind::Cache]); }
        c.normal.postconditions.push(if summary.entry_kind == EntryKind::CallBoundary { Postcondition::CalleeInputsSettled } else { Postcondition::AdmissionStaysCaller });
        if !c.effects.failures.is_empty() {
            let mut unwind = outcome(vec![ResultSlot::service(N::Failure, ServiceType::Failure)]);
            if summary.contract.publication != Publication::Atomic {
                let (name, ty) = match summary.contract.publication { Publication::ChannelReceipt => (N::Receipt, ServiceType::Receipt), _ => (N::Task, ServiceType::TaskState) };
                unwind.results.push(ResultSlot::service(name, ty));
                unwind.postconditions.push(if name == N::Receipt { Postcondition::ReceiptDeterminesHolder } else { Postcondition::TaskDeterminesHolder });
            } else {
                unwind.transfers.push(TransferRule::Settle { inputs: call.packet.into_iter().collect() });
                unwind.postconditions.push(Postcondition::CalleeInputsSettled);
            }
            c.unwind = Some(unwind.clone());
            if c.effects.failures.contains(&FailureKind::Cancelled) {
                c.cancellation = Some(CancellationContract { outcome: unwind, delivery: CancellationDelivery::Unwind });
            }
        }
        if validate && summary.contract.publication != Publication::Atomic && summary.contract.protocol.is_none() { return Err(ContractError::PublicationNeedsProtocol); }
        if let Some(protocol) = summary.contract.protocol { self.protocol(protocol, ProtocolTransition::Publish, c, validate)?; }
        for relation in &summary.results {
            c.normal.postconditions.push(Postcondition::ResultRelation(relation.clone()));
        }
        if call.cleanup == CallCleanupMode::Shielded {
            c.cancellation = None;
            c.effects.failures.retain(|kind| *kind != FailureKind::Cancelled);
            if c.effects.failures.is_empty() {
                c.unwind = None;
            }
            c.normal.postconditions.push(Postcondition::Shielded);
        }
        Ok(())
    }
}
fn outcome(results: Vec<ResultSlot>) -> OutcomeContract { OutcomeContract { results, transfers: vec![], postconditions: vec![], retention: vec![] } }
fn preserve(input: Option<ValueId>, name: SlotName) -> Vec<(ValueId, SlotName)> { input.map(|value| (value, name)).into_iter().collect() }
fn fails(c: &mut OpContract, restored: Vec<(ValueId, SlotName)>, throwing: bool) {
    let mut unwind = outcome(vec![]);
    for (input, name) in restored {
        unwind.results.push(ResultSlot::new(name, ResultType::SameAs(input)));
        unwind.transfers.push(TransferRule::Preserve { input, output: name });
    }
    unwind.results.push(ResultSlot::service(SlotName::Failure, ServiceType::Failure));
    unwind.postconditions.push(Postcondition::NoPublication);
    c.unwind = Some(unwind);
    c.effects.failures = if throwing { all_failures() } else { vec![FailureKind::Program] };
}
fn observation(c: &mut OpContract) {
    c.entry_kind = EntryKind::ObservationAdmissionStaysCaller;
    c.normal.postconditions.extend([Postcondition::AdmissionStaysCaller, Postcondition::PreserveFullDependencies]);
}
fn all_failures() -> Vec<FailureKind> { vec![FailureKind::Thrown, FailureKind::Program, FailureKind::Panic, FailureKind::Native, FailureKind::Cancelled] }
fn captured_cleanup(c: &mut OpContract) {
    cleanup(c);
    c.effects.suspends = false;
    c.effects.failures.clear();
}
fn cleanup(c: &mut OpContract) {
    c.effects.cleanup = true;
    c.effects.suspends = true;
    c.effects.failures = all_failures();
    c.roots.roots = vec![RootKind::Operands, RootKind::ActiveSelf, RootKind::CleanupRemainder, RootKind::PendingExit, RootKind::CarrierGraph, RootKind::Pins, RootKind::FrameSlots, RootKind::Cache, RootKind::ServiceSchedule];
    c.normal.postconditions.extend([Postcondition::Shielded, Postcondition::ContinueCleanup, Postcondition::ServiceSettleBeforeSerialPlan, Postcondition::SerialOrder]);
}
fn state_results(states: TransferStates, outcome: &mut OutcomeContract) {
    match states {
        TransferStates::Same(state) => outcome.results.push(ResultSlot::new(SlotName::State, ResultType::SameAs(state))),
        TransferStates::Between { from, to } => outcome.results.extend([ResultSlot::new(SlotName::From, ResultType::SameAs(from)), ResultSlot::new(SlotName::To, ResultType::SameAs(to))]),
    }
}

fn protocol_states(protocol: ProtocolId, outcome: Outcome, states: Vec<ProtocolPoststate>, target: &mut OutcomeContract) {
    if let [state] = states.as_slice() {
        target.postconditions.push(Postcondition::ProtocolState { holder: state.holder, state: state.state });
    } else if !states.is_empty() {
        target.postconditions.push(Postcondition::ProtocolAlternatives { protocol, outcome, states });
    }
}
