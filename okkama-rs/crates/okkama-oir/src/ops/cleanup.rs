use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarrierDestination { FailureContext(ScopeId), Cache(ValueId), Diagnostic(ValueId) }
impl OperandSource for CarrierDestination {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) {
        match self { Self::FailureContext(_) => {}, Self::Cache(value) | Self::Diagnostic(value) => value.append(mode, out) }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKind { Return, Last, Next, Fallthrough }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitDestination { Caller, Loop(BoundaryId), Scope(ScopeId) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbortPhase { BeforeDestroy, AfterExecutedPrefix }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedExit { pub kind: ExitKind, pub value: Option<ValueId>, pub packet: Option<ValueId>, pub deps: Option<ValueId>, pub selection: SelectionId, pub boundary: BoundaryId }
impl OperandSource for PreparedExit {
    fn append(&self, _: OperandMode, out: &mut Vec<ReferencedOperand>) {
        self.value.append(OperandMode::Borrow, out); self.packet.append(OperandMode::Consume, out); self.deps.append(OperandMode::Observe, out); self.selection.append(OperandMode::Borrow, out);
    }
}

operation_group! { CarrierOp {
    Attach { failure: ValueId, packet: ValueId, owner_state: ValueId, protocol: ProtocolId }
        => ("carrier.attach", [failure: Observe, packet: Consume, owner_state: Consume], Rule::CarrierAttach { failure: *failure, packet: *packet, owner: *owner_state, protocol: *protocol });
    Transfer { state: ValueId, target: CarrierDestination, protocol: ProtocolId }
        => ("carrier.transfer", [state: Consume, target: Consume], Rule::CarrierTransfer { state: *state, target: *target, protocol: *protocol });
    Rehome { state: ValueId, owner_state: ValueId, protocol: ProtocolId }
        => ("carrier.rehome", [state: Consume, owner_state: Consume], Rule::Protocol { state: *state, owner: Some(*owner_state), packet: None, protocol: *protocol, transition: ProtocolTransition::Rehome });
    Release { state: ValueId, owner_state: ValueId, protocol: ProtocolId }
        => ("carrier.release", [state: Consume, owner_state: Consume], Rule::Protocol { state: *state, owner: Some(*owner_state), packet: None, protocol: *protocol, transition: ProtocolTransition::Release });
} }
operation_group! { ExitOp {
    Prepare { exit: PreparedExit }
        => ("exit.prepare", [exit: Borrow], Rule::Prepare(exit.clone()));
    Replace { pending: ValueId, replacement: PreparedExit }
        => ("exit.replace", [pending: Consume, replacement: Borrow], Rule::Replace { pending: *pending, replacement: replacement.clone() });
    Commit { pending: ValueId, destination: ExitDestination, result: Option<TypeId> }
        => ("exit.commit", [pending: Consume], Rule::Commit { pending: *pending, result: *result });
    Extend { pending: ValueId, packet: ValueId }
        => ("exit.extend", [pending: Consume, packet: Consume], Rule::ExtendReservation { pending: *pending,
            packet: *packet });
    Abort { pending: ValueId, cleanup: ValueId }
        => ("exit.abort", [pending: Consume, cleanup: Consume], Rule::Abort { pending: *pending, cleanup: *cleanup });
} }
operation_group! { CleanupOp {
    Begin { state: ValueId, pending: Option<ValueId>, failure: Option<ValueId>, close: CleanupCloseId,
        effects: ScopeEffectEnvelope }
        => ("cleanup.begin", [state: Consume, pending: Consume, failure: Observe],
            Rule::CleanupBegin { state: *state, pending: *pending, failure: *failure,
                effects: effects.clone() });
    SettleEmpty { cleanup: ValueId, schedule: CleanupScheduleId }
        => ("cleanup.settle.empty", [cleanup: Consume],
            Rule::CleanupStep { cleanup: *cleanup, finishing: false });
    Defer { cleanup: ValueId, action: ValueId }
        => ("cleanup.defer", [cleanup: Consume, action: Borrow], Rule::CleanupStep { cleanup: *cleanup, finishing: false });
    Settle { cleanup: ValueId, holder: ValueId, protocol: ProtocolId }
        => ("cleanup.settle", [cleanup: Consume, holder: Consume], Rule::CleanupSettle { cleanup: *cleanup, holder: *holder, protocol: *protocol });
    Plan { cleanup: ValueId, plan: CleanupPlanId }
        => ("cleanup.plan", [cleanup: Consume], Rule::CleanupStep { cleanup: *cleanup, finishing: false });
    Next { cleanup: ValueId }
        => ("cleanup.next", [cleanup: Consume], Rule::CleanupNext(*cleanup));
    Destroy { cleanup: ValueId, key: ValueId }
        => ("cleanup.destroy", [cleanup: Consume, key: Observe],
            Rule::Destroy { cleanup: *cleanup, identity: *key });
    DestroyDirect { cleanup: ValueId, identity: ValueId, descriptor: ValueId, destroy: FuncId }
        => ("destroy.direct", [cleanup: Consume, identity: DestroyBorrow, descriptor: Observe], Rule::Destroy { cleanup: *cleanup, identity: *identity });
    RecordFailure { cleanup: ValueId, failure: ValueId }
        => ("cleanup.failure", [cleanup: Consume, failure: Observe], Rule::CleanupFailure { cleanup: *cleanup });
    Shield { cleanup: ValueId }
        => ("cleanup.shield", [cleanup: Consume], Rule::CleanupStep { cleanup: *cleanup, finishing: false });
    DeliverCancel { cleanup: ValueId }
        => ("cleanup.deliver_cancel", [cleanup: Consume], Rule::CleanupStep { cleanup: *cleanup, finishing: false });
    Finish { cleanup: ValueId }
        => ("cleanup.finish", [cleanup: Consume], Rule::CleanupStep { cleanup: *cleanup, finishing: true });
    CellNew { value: ValueId, ty: TypeId }
        => ("cell.new", [value: Borrow], Rule::Data { ty: ResultType::CellOf(*ty), trap: false });
    CellLoad { cell: ValueId }
        => ("cell.load", [cell: Borrow], Rule::Data { ty: ResultType::CellValue(*cell), trap: false }.requires(Postcondition::NonLinearSlotOnly));
    CellStore { cell: ValueId, value: ValueId }
        => ("cell.store", [cell: Borrow, value: Borrow], Rule::Intent.requires(Postcondition::NonLinearSlotOnly));
} }
