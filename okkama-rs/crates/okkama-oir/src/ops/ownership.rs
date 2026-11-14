use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStates { Same(ValueId), Between { from: ValueId, to: ValueId } }
impl TransferStates {
    pub fn values(self) -> Vec<ValueId> { match self { Self::Same(state) => vec![state], Self::Between { from, to } => vec![from, to] } }
}
impl OperandSource for TransferStates {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) { self.values().append(mode, out); }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicReason { Atomic, Cell, Channel, Group, Any, Closure, Merge }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessContext { pub owner: ValueId, pub holder: ValueId, pub authority: ValueId }
impl OperandSource for AccessContext {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) {
        [self.owner, self.holder, self.authority].to_vec().append(mode, out);
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElaboratedBinding { pub binding: BindingId, pub resource: ResourceValue, pub state: ValueId, pub original_scope: ScopeId }
impl OperandSource for ElaboratedBinding {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) { self.resource.append(mode, out); self.state.append(mode, out); }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElaboratedStage { pub action: ScopeId, pub field: FieldRef, pub resource: ResourceValue, pub states: TransferStates }
impl OperandSource for ElaboratedStage {
    fn append(&self, mode: OperandMode, out: &mut Vec<ReferencedOperand>) { self.resource.append(mode, out); self.states.append(mode, out); }
}

operation_group! { OwnershipOp {
    Empty {  }
        => ("own.empty", [], Rule::Empty);
    Move { packet: ValueId }
        => ("own.move", [packet: Consume], Rule::Move { packets: vec![*packet] });
    Join { packets: Vec<ValueId> }
        => ("own.join", [packets: Consume], Rule::Move { packets: packets.clone() });
    Admit { state: ValueId, packet: ValueId }
        => ("own.admit", [state: Consume, packet: Consume], Rule::Admit { state: *state, packet: *packet, provenance: ProvenanceTransition::Preserve });
    Bind { binding: ElaboratedBinding }
        => ("own.bind", [binding: Consume], Rule::Admit { state: binding.state, packet: binding.resource.packet, provenance: ProvenanceTransition::Set(AdmissionProvenance::Bound) });
    Rebind { binding: ElaboratedBinding }
        => ("own.rebind", [binding: Consume], Rule::Admit { state: binding.state, packet: binding.resource.packet, provenance: ProvenanceTransition::Set(AdmissionProvenance::Bound) });
    Stage { stage: ElaboratedStage }
        => ("own.stage", [stage: Consume], Rule::Stage(stage.clone()));
    Split { packet: ValueId, selection: SelectionId }
        => ("own.split", [packet: Consume, selection: Borrow], Rule::Split { input: *packet, selection: *selection, scope: false, checked: false });
    SplitChecked { packet: ValueId, selection: SelectionId }
        => ("own.split.checked", [packet: Consume, selection: Borrow], Rule::Split { input: *packet, selection: *selection, scope: false, checked: true });
    ScopeBegin { scope: ScopeId }
        => ("scope.begin", [], Rule::ScopeBegin(*scope));
    ScopeSplit { state: ValueId, selection: SelectionId }
        => ("scope.split", [state: Consume, selection: Borrow], Rule::Split { input: *state, selection: *selection, scope: true, checked: true });
    ScopeTransfer { states: TransferStates, selection: SelectionId }
        => ("scope.transfer", [states: Consume, selection: Borrow], Rule::ScopeTransfer { states: *states, selection: *selection });
    Dynamic { state: ValueId, packet: ValueId, target: ScopeId, reason: DynamicReason, protocol: ProtocolId }
        => ("own.dynamic", [state: Consume, packet: Consume], Rule::Dynamic { state: *state, packet: *packet, target: *target, reason: *reason, protocol: *protocol });
    DynamicCheck { key: ValueId, snapshot: ValueId }
        => ("dynamic.check", [key: Observe, snapshot: Observe], Rule::Check);
    DynamicRefresh { key: ValueId, access: AccessContext }
        => ("dynamic.refresh", [key: Observe, access: Observe], Rule::Data { ty: ResultType::Service(ServiceType::OwnerSnapshot), trap: true });
    ScopeEnd { state: ValueId }
        => ("scope.end", [state: Consume], Rule::Close { input: *state, failure: None });
    ScopeUnwind { state: ValueId, failure: ValueId }
        => ("scope.unwind", [state: Consume, failure: Observe], Rule::Close { input: *state, failure: Some(*failure) });
    End { packet: ValueId, boundary: BoundaryId }
        => ("end", [packet: Consume], Rule::Close { input: *packet, failure: None });
} }
