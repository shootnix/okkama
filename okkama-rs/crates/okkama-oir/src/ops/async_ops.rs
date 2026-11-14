use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeBoundary { Existing { owner: ScopeId }, Temporary { awaiting: ScopeId } }
/// Static evidence, never a runtime probe or a use-count heuristic. #564 must
/// check every assertion against the whole function, and repeat after coro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniqueCacheProof {
    pub future: ValueId,
    pub scope: ScopeId,
    pub region: Vec<BlockId>,
    pub evidence: UniqueCacheEvidence,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationOrigin {
    Instruction(InstId),
    Invoke { block: BlockId, outcome: Outcome, slot: u16 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniqueCacheEvidence {
    pub creation: CreationOrigin,
    pub aliases: Vec<ValueId>,
    pub frame_slots: Vec<u32>,
    pub no_external_storage: bool,
    pub no_other_waiters: bool,
    pub no_live_views: bool,
    pub no_collector_retention: bool,
    pub scheduler_cannot_publish: bool,
}
impl OperandSource for UniqueCacheProof {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) {
        self.future.append(OperandMode::Observe, out);
        self.evidence.aliases.append(OperandMode::Observe, out);
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSlot { pub slot: u32, pub value: ValueId, pub mode: OperandMode }
impl OperandSource for FrameSlot {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) { self.value.append(self.mode, out); }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedSlot { pub slot: u32, pub ty: TypeId, pub linear: bool }

operation_group! { AsyncOp {
    Await { future: ValueId, result: Option<TypeId> }
        => ("await", [future: Borrow], Rule::Await { future: *future, result: *result, take: None, shielded: false });
    AwaitShielded { future: ValueId, result: Option<TypeId> }
        => ("await.shielded", [future: Borrow], Rule::Await { future: *future, result: *result, take: None, shielded: true });
    AwaitTake { future: ValueId, result: Option<TypeId>, unique: ValueId, boundary: TakeBoundary }
        => ("await.take", [future: Borrow, unique: Consume], Rule::Await { future: *future, result: *result, take: Some((*unique, *boundary)), shielded: false });
    UniqueCacheProof { proof: UniqueCacheProof }
        => ("unique_cache.proof", [proof: Observe], Rule::UniqueProof(proof.clone()));
    CacheRehome { state: ValueId, target: ScopeId, protocol: ProtocolId }
        => ("cache.rehome", [state: Consume], Rule::Protocol { state: *state, owner: None, packet: None, protocol: *protocol, transition: ProtocolTransition::Rehome }.requires(Postcondition::CacheRehome(*target)));
    FutureSpawn { future: ValueId }
        => ("fut.spawn", [future: Borrow], Rule::Data { ty: ResultType::SameAs(*future), trap: false });
    FutureCancel { future: ValueId }
        => ("fut.cancel", [future: Observe], Rule::Intent);
    FutureDone { future: ValueId }
        => ("fut.done", [future: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FutureCancelled { future: ValueId }
        => ("fut.cancelled", [future: Observe], Rule::Data { ty: ResultType::Scalar(ScalarType::Bool), trap: false });
    FutureNew { frame: ValueId, result: Option<TypeId>, packet: ValueId, deps: ValueId, owner: ScopeId, owner_state: ValueId }
        => ("fut.new", [frame: Consume, packet: Consume, deps: Observe, owner_state: Consume], Rule::FutureNew { frame: *frame, packet: *packet, owner: *owner_state, result: *result });
    FuturePublish { task: ValueId, cache: ValueId, value: Option<ValueId>, packet: ValueId, deps: ValueId, failure: Option<ValueId>, protocol: ProtocolId }
        => ("fut.publish", [task: Consume, cache: Consume, value: Borrow, packet: Consume, deps: Observe, failure: Observe], Rule::Publish { task: *task, cache: *cache, packet: *packet, protocol: *protocol });
    FutureSettle { task: ValueId, protocol: ProtocolId }
        => ("fut.settle", [task: Consume], Rule::TaskSettle { task: *task, protocol: *protocol });
    FrameAlloc { function: FuncId }
        => ("frame.alloc", [], Rule::Data { ty: ResultType::Frame(*function), trap: false });
    FrameLoad { frame: ValueId, slot: u32, ty: TypeId }
        => ("frame.load", [frame: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: false }.requires(Postcondition::NonLinearSlotOnly));
    FrameStore { frame: ValueId, slot: u32, value: ValueId }
        => ("frame.store", [frame: Borrow, value: Borrow], Rule::Intent.requires(Postcondition::NonLinearSlotOnly));
    FrameSave { frame: ValueId, slots: Vec<FrameSlot>, continuation: u32 }
        => ("frame.save", [frame: Consume, slots: Observe], Rule::FrameSave { frame: *frame, slots: slots.clone() });
    FrameResume { frame: ValueId, slots: Vec<SavedSlot> }
        => ("frame.resume", [frame: Consume], Rule::FrameResume { frame: *frame, slots: slots.clone() });
    FrameCancel { frame: ValueId, cleanup: ValueId, protocol: ProtocolId }
        => ("frame.cancel", [frame: Consume, cleanup: Consume], Rule::FrameCancel { frame: *frame, cleanup: *cleanup, protocol: *protocol });
    Pin { value: ValueId, deps: ValueId, owner: ValueId, protocol: ProtocolId }
        => ("pin", [value: Borrow, deps: Observe, owner: Consume], Rule::Pin { owner: *owner, protocol: *protocol });
    PinTransfer { pin: ValueId, owner: ValueId, protocol: ProtocolId }
        => ("pin.transfer", [pin: Consume, owner: Consume], Rule::Protocol { state: *pin, owner: Some(*owner), packet: None, protocol: *protocol, transition: ProtocolTransition::Transfer });
    Unpin { pin: ValueId, owner: ValueId, protocol: ProtocolId }
        => ("unpin", [pin: Consume, owner: Consume], Rule::Protocol { state: *pin, owner: Some(*owner), packet: None, protocol: *protocol, transition: ProtocolTransition::Unpin });
    WorkerBind { action: ValueId, entry: ValueId, fields: Vec<ValueId>, ty: TypeId }
        => ("worker.bind", [action: Borrow, entry: Borrow, fields: Borrow], Rule::Data { ty: ResultType::Exact(*ty), trap: true });
    WorkerSpawn { entry: ValueId, args: Vec<ValueId>, direct: bool, result: Option<TypeId> }
        => ("worker.spawn", [entry: Borrow, args: Borrow], Rule::Worker(*result));
    ProcessSpawn { constructor: FuncId, constructor_args: Vec<ValueId>, action: FuncId, args: Vec<ValueId>, result: Option<TypeId> }
        => ("proc.spawn", [constructor_args: Borrow, args: Borrow], Rule::Worker(*result));
} }
