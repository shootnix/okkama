use crate::*;
use crate::cfg::*;
use std::collections::HashSet;

pub(crate) trait Sample { fn sample() -> Self; }
pub(crate) fn sample<T: Sample>() -> T { T::sample() }
macro_rules! id_samples { ($($ty:ty),*) => { $( impl Sample for $ty { fn sample() -> Self { Self::from_u32(0) } } )* }; }
id_samples!(
    CleanupCloseId, CleanupPlanId, CleanupScheduleId, RootMapId, ValueId, BlockId, InstId, TypeId, DeclId, FuncId,
    ScopeId, BindingId, BoundaryId, SummaryId, ProtocolId, SelectionId, DepExprId, KeyExprId, CreationSiteId
);
impl Sample for u32 { fn sample() -> Self { 0 } }
impl Sample for bool { fn sample() -> Self { true } }
impl Sample for String { fn sample() -> Self { "diagnostic".into() } }
impl<T: Sample> Sample for Vec<T> { fn sample() -> Self { vec![sample()] } }
impl<T: Sample> Sample for Option<T> { fn sample() -> Self { Some(sample()) } }
impl<A: Sample, B: Sample> Sample for (A, B) { fn sample() -> Self { (sample(), sample()) } }
macro_rules! sample_expr { ($ty:ty, $expr:expr) => { impl Sample for $ty { fn sample() -> Self { $expr } } }; }
sample_expr!(NumericType, NumericType::Int);
sample_expr!(FieldRef, FieldRef { declaration: sample(), index: 0 });
sample_expr!(ConstructionStatus, ConstructionStatus::Complete);
sample_expr!(ConstructionCompletion, ConstructionCompletion { state: sample() });
sample_expr!(FieldValue, FieldValue { field: sample(), value: sample() });
sample_expr!(ResourceValue, ResourceValue { value: sample(), packet: sample() });
sample_expr!(ResultDemand, ResultDemand::Discard);
sample_expr!(DeferAction, DeferAction { replacements: vec![], function: sample(), captures: sample(),
    bindings: sample() });
sample_expr!(TransferStates, TransferStates::Same(sample()));
sample_expr!(ElaboratedBinding, ElaboratedBinding { binding: sample(), resource: sample(), state: sample(), original_scope: sample() });
sample_expr!(ElaboratedStage, ElaboratedStage { action: sample(), field: sample(), resource: sample(), states: sample() });
sample_expr!(AccessContext, AccessContext { owner: sample(), holder: sample(), authority: sample() });
sample_expr!(DynamicReason, DynamicReason::Any);
sample_expr!(TakeBoundary, TakeBoundary::Temporary { awaiting: sample() });
sample_expr!(CreationOrigin, CreationOrigin::Instruction(sample()));
sample_expr!(UniqueCacheProof, UniqueCacheProof { future: sample(), scope: sample(), region: sample(), evidence: UniqueCacheEvidence {
    creation: sample(), aliases: vec![], frame_slots: vec![], no_external_storage: true, no_other_waiters: true, no_live_views: true, no_collector_retention: true, scheduler_cannot_publish: true,
} });
sample_expr!(FrameSlot, FrameSlot { slot: 0, value: sample(), mode: OperandMode::Consume });
sample_expr!(SavedSlot, SavedSlot { slot: 0, ty: sample(), linear: true });
sample_expr!(CarrierDestination, CarrierDestination::Cache(sample()));
sample_expr!(PreparedExit, PreparedExit { kind: ExitKind::Return, value: sample(), packet: sample(), deps: sample(), selection: sample(), boundary: sample() });
sample_expr!(ExitDestination, ExitDestination::Caller);
sample_expr!(AbortPhase, AbortPhase::BeforeDestroy);
sample_expr!(CallArgument, CallArgument { value: sample(), convention: OperandConvention::Borrow });
sample_expr!(CallSite, CallSite { cleanup: CallCleanupMode::Ordinary,  args: vec![CallArgument { value: sample(),
    convention: OperandConvention::Borrow }], packet: sample(), deps: sample(), descriptors: sample(),
    type_args: sample(), summary: sample() });
sample_expr!(DescriptorContract, DescriptorContract { managed: DescriptorManaged::Runtime, retention: SummarySelection::Parameter(0), cleanup: sample(), async_destroy: false, summary: sample() });
fn v(index: u32) -> ValueId { ValueId::from_u32(index) }
pub(crate) fn module() -> Module {
    let mut module = Module {
        name: "contracts".into(), version: None, profile: Profile::Own, requires: vec![Feature::Core], imports: vec![],
        declarations: Arena::new(), types: TypeTable::new(), scopes: Arena::new(), summaries: Arena::new(), protocols: Arena::new(), functions: Arena::new(), entry: None, tests: None, files: Arena::new(),
    };
    module.types.intern(Type::Int).unwrap();
    module.summaries.alloc(Summary {
        entry_kind: EntryKind::CallBoundary,
        parameters: vec![ParameterEffect { convention: FreshConvention::Borrow, retention: None, deps: vec![], destination: TransferDestination::None }],
        results: vec![ResultRelation { fresh: vec![], aliases: vec![], external_deps: vec![], provenance: AdmissionProvenance::UnboundResult }],
        contract: CallContract { results: vec![sample()], failures: vec![FailureKind::Thrown, FailureKind::Cancelled], suspends: false, async_destroy: false, publication: Publication::Atomic, protocol: None },
    }).unwrap();
    module.protocols.alloc(ProtocolDecl { kind: ProtocolKind::Storage,
        holders: vec![HolderKind::Cache, HolderKind::Frame, HolderKind::Carrier, HolderKind::Scope],
        rules: [ProtocolTransition::Attach, ProtocolTransition::Transfer, ProtocolTransition::Rehome, ProtocolTransition::Release, ProtocolTransition::Publish, ProtocolTransition::Settle, ProtocolTransition::Save, ProtocolTransition::Resume, ProtocolTransition::Cancel, ProtocolTransition::Pin, ProtocolTransition::Unpin, ProtocolTransition::Dynamic].into_iter().map(|transition| ProtocolRule {
            transition, from: HolderKind::Scope, to: HolderKind::Cache, checks: vec![ProtocolCheck::LiveDependencies], normal: ProtocolPoststate { holder: HolderKind::Cache, state: HolderState::Open }, unwind: Some(ProtocolPoststate { holder: HolderKind::Receipt, state: HolderState::Unaccepted }), cancel: Some(ProtocolPoststate { holder: HolderKind::Frame, state: HolderState::Running }),
        }).collect(), roots: vec![RootKind::Cache],
    }).unwrap();
    module
}
fn body() -> Body {
    let mut body = Body { cleanup: CleanupMetadata::default(), high_results: vec![], high_effects: vec![],
        descriptor_bindings: vec![],
        service_schedules: vec![], entry: sample(), values: Arena::new(), blocks: Arena::new(), insts: Arena::new(),
        bindings: Arena::new(), boundaries: Arena::new(), deps: Arena::new(), keys: Arena::new(),
        selections: Arena::new() };
    body.selections.alloc(SelectionExpr::Value(v(0))).unwrap();
    body
}
pub(crate) fn representatives() -> Vec<Operation> {
    let mut ops = vec![Operation::Const(Literal::Int("1".into()))];
    ops.extend(CoreOp::representatives().into_iter().map(|mut op| {
        if let CoreOp::Convert { to, .. } = &mut op { *to = NumericType::Rat; }
        Operation::Core(op)
    }));
    ops.extend(LexicalOp::representatives().into_iter().map(Operation::Lexical));
    ops.extend(OwnershipOp::representatives().into_iter().map(Operation::Ownership));
    ops.extend(CallOp::representatives().into_iter().map(|mut op| {
        match &mut op { CallOp::Role { call, .. } | CallOp::Closure { call, .. } | CallOp::Cap { call, .. } => call.args.clear(), _ => {} }
        Operation::Call(op)
    }));
    ops.extend(CollectionOp::representatives().into_iter().map(Operation::Collection));
    ops.extend(AsyncOp::representatives().into_iter().map(Operation::Async));
    ops.extend(CarrierOp::representatives().into_iter().map(Operation::Carrier));
    ops.extend(ExitOp::representatives().into_iter().map(Operation::Exit));
    ops.extend(CleanupOp::representatives().into_iter().map(Operation::Cleanup));
    ops
}
fn names(outcome: &OutcomeContract) -> Vec<SlotName> { outcome.results.iter().map(|slot| slot.name).collect() }

#[test]
fn every_operation_has_contract_and_traversal() {
    let module = module(); let registry = ContractRegistry::new(&module);
    let mut body = body(); let mut seen = HashSet::new();
    for op in representatives() {
        assert!(seen.insert(op.opcode()), "duplicate opcode {}", op.opcode());
        let contract = registry.lookup(&op).unwrap_or_else(|err| panic!("{}: {err}", op.opcode()));
        assert_eq!(contract.operands, op.operand_refs());
        assert_eq!(op.operand_refs().iter().map(|operand| operand.reference).collect::<Vec<_>>(), op.declared_operands(), "{} omitted field", op.opcode());
        let uses = operation_uses(&op, &body).unwrap();
        assert!(uses.len() >= contract.operands.len());
        // Each representative is placed in an actual hand-built function body.
        let inst = body.insts.alloc(Instruction { op, results: vec![], facts: InstructionFacts::default(), loc: None }).unwrap();
        if body.blocks.is_empty() { body.blocks.alloc(Block { lexical_scope: sample(), params: vec![], unwind: None, insts: vec![], term: Terminator { kind: TermKind::Unreachable, loc: None } }).unwrap(); }
        body.blocks.get_mut(body.entry).unwrap().insts.push(inst);
        for (outcome, schema) in [(Outcome::Normal, Some(&contract.normal)), (Outcome::Unwind, contract.unwind.as_ref()), (Outcome::Cancel, contract.cancellation.as_ref().map(|cancel| &cancel.outcome))] {
            if let Some(schema) = schema {
                let mut slots = HashSet::new();
                for (index, slot) in schema.results.iter().enumerate() {
                    assert!(slots.insert(format!("{:?}", slot.name)), "{} duplicate slot", body.insts.get(inst).unwrap().op.opcode());
                    assert_eq!(contract.edge_result(EdgeResult { outcome, slot: index as u16 }), Some(slot));
                }
            }
        }
    }
    assert_eq!(seen, Operation::opcode_inventory().into_iter().collect());
    let mut visits = HashSet::new();
    walk_body(&body, registry, |event| { if let CfgEvent::Use { site: UseSite::Instruction(id), .. } = event { visits.insert(id); } }).unwrap();
    for (id, inst) in body.insts.iter() { assert_eq!(visits.contains(&id), !inst.op.operand_refs().is_empty(), "{} traversal", inst.op.opcode()); }
}

#[test]
fn representative_group_contracts() {
    use SlotName as N;
    let module = module(); let registry = ContractRegistry::new(&module);
    let split = registry.lookup(&Operation::Ownership(OwnershipOp::SplitChecked { packet: v(1), selection: sample() })).unwrap();
    assert_eq!(names(&split.normal), [N::Rest, N::Selected, N::ExternalDeps]);
    assert_eq!(names(split.unwind.as_ref().unwrap()), [N::Packet, N::Failure]);
    assert_eq!(split.unwind.as_ref().unwrap().transfers, [TransferRule::Preserve { input: v(1), output: N::Packet }]);
    let end = registry.lookup(&Operation::Ownership(OwnershipOp::ScopeEnd { state: v(1), reserve: None,
        effects: ScopeEffectEnvelope::conservative() })).unwrap();
    assert_eq!(names(&end.normal), [N::Closed]);
    assert_eq!(names(end.unwind.as_ref().unwrap()), [N::Closed, N::Failure]);
    assert!(end.cancellation.is_none());
    let unwind = registry.lookup(&Operation::Ownership(OwnershipOp::ScopeUnwind { state: v(1), failure: v(2),
        reserve: None, effects: ScopeEffectEnvelope::conservative() })).unwrap();
    assert_eq!(names(&unwind.normal), [N::Failure]); assert!(unwind.unwind.is_none());
    assert!(unwind.normal.postconditions.contains(&Postcondition::Closed(v(1))));
    for reason in [DynamicReason::Atomic, DynamicReason::Cell, DynamicReason::Channel, DynamicReason::Group, DynamicReason::Any, DynamicReason::Closure, DynamicReason::Merge] {
        let dynamic = registry.lookup(&Operation::Ownership(OwnershipOp::Dynamic { state: v(1), packet: v(2), target: sample(), reason, protocol: sample() })).unwrap();
        assert_eq!(names(&dynamic.normal), [N::State]);
        assert_eq!(names(dynamic.unwind.as_ref().unwrap()), [N::State, N::Packet, N::Failure]);
        assert!(dynamic.normal.transfers.iter().any(|rule| matches!(rule, TransferRule::Protocol { transition: ProtocolTransition::Dynamic, .. })));
    }
    let take = registry.lookup(&Operation::Async(AsyncOp::AwaitTake { future: v(1), result: Some(sample()), unique: v(2), boundary: TakeBoundary::Existing { owner: sample() } })).unwrap();
    assert_eq!(take.operands[1], ReferencedOperand { reference: OperandRef::Value(v(2)), mode: OperandMode::Consume });
    assert!(take.normal.postconditions.contains(&Postcondition::CacheConsumed(v(1))));
    assert!(take.normal.postconditions.contains(&Postcondition::FreshAdmission(AdmissionProvenance::ExtractedAtExistingBoundary)));
    assert_eq!(names(take.unwind.as_ref().unwrap()), [N::Failure, N::Task, N::Receipt]);
    assert_eq!(take.cancellation.as_ref().unwrap().outcome, *take.unwind.as_ref().unwrap());
    let proof = registry.lookup(&Operation::Async(AsyncOp::UniqueCacheProof { proof: sample() })).unwrap();
    assert_eq!(proof.normal.results, [ResultSlot::service(N::Unique, ServiceType::UniqueCache)]);
    assert!(proof.normal.postconditions.contains(&Postcondition::UniqueCacheRecheckAfterCoro));
    let push = registry.lookup(&Operation::Collection(CollectionOp::BuilderPush { buffer: v(1), value: v(2), packet: Some(v(3)) })).unwrap();
    assert_eq!(names(push.unwind.as_ref().unwrap()), [N::Buffer, N::Packet, N::Failure]);
    let stage = registry.lookup(&Operation::Lexical(LexicalOp::Stage { action: sample(), field: sample(), value: v(1) })).unwrap();
    assert_eq!(stage.operands.len(), 1); assert!(stage.effects.mutates);
    let prepare = registry.lookup(&Operation::Exit(ExitOp::Prepare { exit: sample() })).unwrap();
    assert_eq!(prepare.normal.results[0].ty, ResultType::Service(ServiceType::Pending));
    let carrier = registry.lookup(&Operation::Carrier(CarrierOp::Attach { failure: v(1), packet: v(2), owner_state: v(3), protocol: sample() })).unwrap();
    assert_eq!(names(carrier.unwind.as_ref().unwrap()), [N::Packet, N::To, N::Failure]);
    let destroy = registry.lookup(&Operation::Cleanup(CleanupOp::DestroyDirect { cleanup: v(1), identity: v(2), descriptor: v(3), destroy: sample() })).unwrap();
    assert_eq!(destroy.operands[1].mode, OperandMode::DestroyBorrow);
    assert!(destroy.unwind.is_none()); assert!(destroy.roots.roots.contains(&RootKind::CleanupRemainder));
    let eq = registry.lookup(&Operation::Core(CoreOp::ValEq { left: v(1), right: v(2) })).unwrap();
    assert_eq!(eq.entry_kind, EntryKind::ObservationAdmissionStaysCaller);
}

#[test]
fn call_conventions_summaries_and_edge_results() {
    let mut module = module();
    module.summaries.get_mut(sample()).unwrap().parameters.push(ParameterEffect { convention: FreshConvention::Take, retention: Some(SummarySelection::OwnerGraph(1)), deps: vec![1], destination: TransferDestination::Result });
    let call = CallSite { cleanup: CallCleanupMode::Ordinary,  args: vec![CallArgument { value: v(1),
        convention: OperandConvention::Borrow }, CallArgument { value: v(2),
        convention: OperandConvention::Consume }], packet: Some(v(3)), deps: vec![v(4)], descriptors: vec![v(5)],
        type_args: vec![], summary: sample() };
    let registry = ContractRegistry::new(&module);
    let contract = registry.lookup(&Operation::Call(CallOp::Direct { function: sample(), call: call.clone() })).unwrap();
    assert_eq!(contract.operands.iter().map(|operand| operand.mode).collect::<Vec<_>>(), [OperandMode::Borrow, OperandMode::Consume, OperandMode::Consume, OperandMode::Observe, OperandMode::Observe]);
    assert_eq!(names(&contract.normal), [SlotName::Value, SlotName::Packet, SlotName::Deps]);
    assert_eq!(names(contract.unwind.as_ref().unwrap()), [SlotName::Failure]);
    assert_eq!(contract.edge_result(EdgeResult { outcome: Outcome::Normal, slot: 1 }).unwrap().ty, ResultType::Service(ServiceType::Own));
    assert!(contract.edge_result(EdgeResult { outcome: Outcome::Unwind, slot: 1 }).is_none());
    let mut wrong = call; wrong.args[1].convention = OperandConvention::Borrow;
    assert_eq!(registry.lookup(&Operation::Call(CallOp::Direct { function: sample(), call: wrong })), Err(ContractError::CallConvention { argument: 1 }));
}

#[test]
fn cfg_visits_all_operands_edges_and_definitions_of_hand_built_function() {
    let module = module(); let registry = ContractRegistry::new(&module); let mut body = body();
    *body.selections.get_mut(sample()).unwrap() = SelectionExpr::Projection(Projection::Slice { value: v(4), start: v(5), end: v(6) });
    body.keys.alloc(KeyExpr::Created { site: sample(), instance: v(7) }).unwrap();
    body.deps.alloc(DepExpr::Key(sample())).unwrap();
    for index in 0..20 { body.values.alloc(ValueData { ty: sample(), def: Definition::BlockParam { block: sample(), index }, facts: ValueFacts::Service }).unwrap(); }
    body.values.get_mut(v(4)).unwrap().facts = ValueFacts::Managed { deps: sample() };
    let inst = body.insts.alloc(Instruction {
        results: vec![v(10), v(11), v(12)], op: Operation::Ownership(OwnershipOp::SplitChecked { packet: v(3), selection: sample() }),
        facts: InstructionFacts { result_deps: vec![(0, sample())], retention: vec![sample()], provenance: vec![] }, loc: None,
    }).unwrap();
    let target = |n, args| Target { block: BlockId::from_u32(n), args };
    let invoke_target = |n, args| InvokeEdge { block: BlockId::from_u32(n), args };
    body.blocks.alloc(Block {
        lexical_scope: sample(), params: vec![v(0), v(1)], unwind: Some(BlockUnwind { target: target(2, vec![v(1)]) }), insts: vec![inst],
        term: Terminator { loc: None, kind: TermKind::InvokeOwn {
            op: Box::new(Operation::Ownership(OwnershipOp::ScopeEnd { state: v(2), reserve: None,
                effects: ScopeEffectEnvelope::conservative() })),
            normal: invoke_target(1, vec![EdgeArg::Value(v(8)), EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot: 0 })]),
            unwind: invoke_target(2, vec![EdgeArg::Value(v(9)), EdgeArg::Produced(EdgeResult { outcome: Outcome::Unwind, slot: 0 }), EdgeArg::Produced(EdgeResult { outcome: Outcome::Unwind, slot: 1 })]),
        } },
    }).unwrap();
    body.blocks.alloc(Block { lexical_scope: sample(), params: vec![v(13)], unwind: None, insts: vec![], term: Terminator { loc: None, kind: TermKind::Ret { values: vec![v(13)] } } }).unwrap();
    body.blocks.alloc(Block { lexical_scope: sample(), params: vec![v(14)], unwind: None, insts: vec![], term: Terminator { loc: None, kind: TermKind::Raise(v(14)) } }).unwrap();
    let function = Function { symbol: "hand_built".into(), signature: Signature { params: vec![], results: vec![], ownership_abi: OwnershipAbi::Unelaborated }, attrs: vec![], type_params: vec![], summary: None, async_creation_summary: None, async_body_summary: None, body: Some(body.clone()), loc: None };
    let mut events = vec![]; walk_function(&function, registry, |event| events.push(event)).unwrap();
    let uses = |site| events.iter().filter_map(|event| match event { CfgEvent::Use { site: actual, operand } if *actual == site => Some(operand.value), _ => None }).collect::<Vec<_>>();
    assert_eq!(uses(UseSite::Instruction(inst)), [v(3), v(4), v(5), v(6)]);
    assert_eq!(uses(UseSite::InstructionFacts(inst)), [v(7), v(4), v(5), v(6)]);
    assert_eq!(uses(UseSite::ValueFacts(v(4))), [v(7)]);
    assert_eq!(uses(UseSite::BlockUnwind(sample())), [v(1)]);
    assert_eq!(uses(UseSite::Terminator(sample())), [v(8), v(9), v(2)]);
    assert_eq!(events.iter().filter(|event| matches!(event, CfgEvent::Definition { .. })).count(), 7);
    assert_eq!(events.iter().filter(|event| matches!(event, CfgEvent::EdgeDefinition { .. })).count(), 4);
    assert_eq!(events.iter().filter(|event| matches!(event, CfgEvent::EdgeResultUse { .. })).count(), 3);
    assert_eq!(successors(sample(), &body, registry).unwrap(), [BlockId::from_u32(1), BlockId::from_u32(2)].into_iter().collect());
}

#[test]
fn provenance_retention_and_high_buffer_shapes() {
    let module = module(); let registry = ContractRegistry::new(&module);
    let admit = registry.lookup(&Operation::Ownership(OwnershipOp::Admit { state: v(1), packet: v(2) })).unwrap();
    assert!(admit.normal.postconditions.contains(&Postcondition::PreserveProvenance));
    assert!(!admit.normal.postconditions.iter().any(|fact| matches!(fact, Postcondition::FreshAdmission(_))));
    let get = registry.lookup(&Operation::Core(CoreOp::ObjGet { object: v(1), field: sample() })).unwrap();
    assert_eq!(get.normal.retention, [RetentionRule { result: SlotName::Value, selection: Retention::Field { base: v(1), field: sample() } }]);
    let cap = registry.lookup(&Operation::Core(CoreOp::CapMake { owner: v(1), ty: sample(), packet: Some(v(2)) })).unwrap();
    assert_eq!(cap.normal.retention[0].selection, Retention::OwnerGraph(v(1)));
    for (op, expected) in [
        (CollectionOp::BuilderPush { buffer: v(1), value: v(2), packet: None }, vec![SlotName::Buffer]),
        (CollectionOp::BuilderFinish { buffer: v(1), packet: None }, vec![SlotName::Value]),
        (CollectionOp::BuilderAbandon { buffer: v(1), packet: None }, vec![]),
    ] { assert_eq!(names(&registry.lookup(&Operation::Collection(op)).unwrap().normal), expected); }
    for states in [TransferStates::Same(v(1)), TransferStates::Between { from: v(1), to: v(2) }] {
        let op = Operation::Ownership(OwnershipOp::ScopeTransfer { states, selection: sample() });
        let contract = registry.lookup(&op).unwrap();
        assert_eq!(contract.normal.results.len(), states.values().len());
        assert_eq!(operation_uses(&op, &body()).unwrap().len(), states.values().len() + 1);
    }
    let pure = registry.lookup(&Operation::Core(CoreOp::Convert { value: v(1), from: NumericType::Int, to: NumericType::Rat })).unwrap();
    assert!(pure.unwind.is_none());
    let checked = registry.lookup(&Operation::Core(CoreOp::Convert { value: v(1), from: NumericType::Rat, to: NumericType::Float })).unwrap();
    assert!(checked.unwind.is_some());
}

#[test]
fn missing_declarations_and_protocol_poststates_are_explicit() {
    let mut module = module();
    let missing = Operation::Call(CallOp::Intrinsic { function: sample(), call: CallSite { summary: Some(SummaryId::from_u32(100)), ..sample() } });
    assert!(matches!(ContractRegistry::new(&module).lookup(&missing), Err(ContractError::InvalidId(_))));
    let dynamic = Operation::Ownership(OwnershipOp::Dynamic { state: v(1), packet: v(2), target: sample(), reason: DynamicReason::Channel, protocol: ProtocolId::from_u32(100) });
    assert!(matches!(ContractRegistry::new(&module).lookup(&dynamic), Err(ContractError::InvalidId(_))));
    module.summaries.get_mut(sample()).unwrap().contract.publication = Publication::ChannelReceipt;
    let op = Operation::Call(CallOp::Intrinsic { function: sample(), call: sample() });
    assert_eq!(ContractRegistry::new(&module).lookup(&op), Err(ContractError::PublicationNeedsProtocol));
    module.summaries.get_mut(sample()).unwrap().contract.protocol = Some(sample());
    let contract = ContractRegistry::new(&module).lookup(&op).unwrap();
    assert_eq!(names(contract.unwind.as_ref().unwrap()), [SlotName::Failure, SlotName::Receipt]);
    assert_eq!(names(&contract.cancellation.as_ref().unwrap().outcome), names(contract.unwind.as_ref().unwrap()));
    assert!(contract.cancellation.as_ref().unwrap().outcome.postconditions.contains(&Postcondition::ProtocolState { holder: HolderKind::Frame, state: HolderState::Running }));
    // The receipt encapsulates unaccepted ownership or identifies the channel;
    // no unconditional restoration of the consumed input packet is promised.
    assert!(!contract.unwind.as_ref().unwrap().transfers.iter().any(|rule| matches!(rule, TransferRule::Preserve { .. })));
    let proof = Operation::Async(AsyncOp::UniqueCacheProof { proof: UniqueCacheProof {
        future: v(1), evidence: UniqueCacheEvidence { aliases: vec![v(2), v(3)], ..sample::<UniqueCacheProof>().evidence }, ..sample()
    } });
    assert_eq!(operation_uses(&proof, &body()).unwrap().iter().map(|use_| use_.value).collect::<Vec<_>>(), [v(1), v(2), v(3)]);
    *module.protocols.get_mut(sample()).unwrap() = ProtocolDecl { kind: ProtocolKind::Storage,  holders: vec![],
        rules: vec![], roots: vec![] };
    assert!(matches!(ContractRegistry::new(&module).lookup(&op), Err(ContractError::MissingTransition { .. })));
}

#[test]
fn every_terminator_and_expression_shape_is_traversed() {
    let module = module(); let registry = ContractRegistry::new(&module); let mut body = body();
    let target = Target { block: BlockId::from_u32(1), args: vec![v(10)] };
    let produced = InvokeEdge { block: BlockId::from_u32(2), args: vec![EdgeArg::Value(v(11)), EdgeArg::Produced(EdgeResult { outcome: Outcome::Normal, slot: 0 })] };
    for (term, expected, edges) in [
        (TermKind::Br(target.clone()), vec![v(10)], 1),
        (TermKind::Cond { condition: v(1), yes: target.clone(), no: target.clone() }, vec![v(10), v(10), v(1)], 2),
        (TermKind::Switch { value: v(2), cases: vec![SwitchCase { value: Literal::Bool(true), target: target.clone() }], default: target.clone() }, vec![v(10), v(10), v(2)], 2),
        (TermKind::Ret { values: vec![v(3), v(4)] }, vec![v(3), v(4)], 0),
        (TermKind::Throw(v(5)), vec![v(5)], 0), (TermKind::Raise(v(6)), vec![v(6)], 0), (TermKind::Panic(v(7)), vec![v(7)], 0),
        (TermKind::Catch { failure: v(8), ty: sample(), handler: produced.clone(), next: target.clone() }, vec![v(11), v(10), v(8)], 2),
        (TermKind::Lookup { kind: LookupKind::Array, collection: v(1), key: v(2), found: produced.clone(), missing: target.clone() }, vec![v(11), v(10), v(1), v(2)], 2),
        (TermKind::Lookup { kind: LookupKind::Map, collection: v(3), key: v(4), found: produced, missing: target }, vec![v(11), v(10), v(3), v(4)], 2),
        (TermKind::Trap { code: TrapCode::Confirm, message: "stop".into() }, vec![], 0), (TermKind::Unreachable, vec![], 0),
    ] {
        let mut events = vec![];
        walk_terminator(sample(), &Terminator { kind: term, loc: None }, &body, registry, &mut |event| events.push(event)).unwrap();
        assert_eq!(events.iter().filter_map(|event| if let CfgEvent::Use { operand, .. } = event { Some(operand.value) } else { None }).collect::<Vec<_>>(), expected);
        assert_eq!(events.iter().filter(|event| matches!(event, CfgEvent::Edge { .. })).count(), edges);
    }
    let selections = [
        (SelectionExpr::Value(v(1)), vec![v(1)]), (SelectionExpr::OwnerGraph(v(2)), vec![v(2)]), (SelectionExpr::ViewOwner(v(3)), vec![v(3)]),
        (SelectionExpr::Projection(Projection::Field { value: v(4), field: sample() }), vec![v(4)]),
        (SelectionExpr::Projection(Projection::Index { value: v(5), index: v(6) }), vec![v(5), v(6)]),
        (SelectionExpr::Projection(Projection::Map { value: v(7), keys: v(8) }), vec![v(7), v(8)]),
        (SelectionExpr::Descriptor { descriptor: v(9), value: v(10) }, vec![v(9), v(10)]),
        (SelectionExpr::Union(vec![sample()]), vec![v(0)]),
    ];
    for (selection, expected) in selections {
        let id = body.selections.alloc(selection).unwrap();
        assert_eq!(operation_uses(&Operation::Ownership(OwnershipOp::Split { packet: v(12), selection: id }), &body).unwrap().iter().skip(1).map(|use_| use_.value).collect::<Vec<_>>(), expected);
    }
    let cycle = body.selections.alloc(SelectionExpr::Union(vec![])).unwrap();
    *body.selections.get_mut(cycle).unwrap() = SelectionExpr::Union(vec![cycle]);
    assert_eq!(operation_uses(&Operation::Ownership(OwnershipOp::Split { packet: v(1), selection: cycle }), &body), Err(TraversalError::CyclicExpression(OperandRef::Selection(cycle))));
}

// Independent of the opcode's explicit use list: enumerate every declared
// payload field, so forgetting a ValueId in a new traversal fails completeness.
pub(crate) trait DeclaredOperands { fn append_declared(&self, out: &mut Vec<OperandRef>); }
macro_rules! metadata_fields { ($($ty:ty),*) => { $( impl DeclaredOperands for $ty { fn append_declared(&self, _: &mut Vec<OperandRef>) {} } )* }; }
metadata_fields!(
    CleanupCloseId, CleanupPlanId, CleanupScheduleId, RootMapId, u32, bool, String, TypeId, DeclId, FuncId,
    ScopeId, BindingId, BoundaryId, SummaryId, ProtocolId, NumericType, FieldRef, DescriptorContract, ResultDemand,
    TakeBoundary, SavedSlot, ExitDestination, AbortPhase, DynamicReason, ConstructionStatus
);
macro_rules! ssa_fields { ($($ty:ty),*) => { $( impl DeclaredOperands for $ty {
    fn append_declared(&self, out: &mut Vec<OperandRef>) {
        let mut operands = vec![]; crate::ops::OperandSource::append(self, OperandMode::Borrow, &mut operands);
        out.extend(operands.into_iter().map(|operand| operand.reference));
    }
} )* }; }
ssa_fields!(ValueId, SelectionId, DepExprId, KeyExprId, FieldValue, ResourceValue, ElaboratedBinding, ElaboratedStage, TransferStates, DeferAction, AccessContext, CallArgument, CallSite, UniqueCacheProof, FrameSlot, CarrierDestination, PreparedExit, ConstructionCompletion);
impl<T: DeclaredOperands> DeclaredOperands for Vec<T> { fn append_declared(&self, out: &mut Vec<OperandRef>) { for value in self { value.append_declared(out); } } }
impl<T: DeclaredOperands> DeclaredOperands for Option<T> { fn append_declared(&self, out: &mut Vec<OperandRef>) { if let Some(value) = self { value.append_declared(out); } } }
impl<A: DeclaredOperands, B: DeclaredOperands> DeclaredOperands for (A, B) { fn append_declared(&self, out: &mut Vec<OperandRef>) { self.0.append_declared(out); self.1.append_declared(out); } }

#[test]
fn construction_high_exit_and_taken_outcome_keep_distinct_contracts() {
    let module = module(); let registry = ContractRegistry::new(&module);
    assert!(ServiceType::UniqueCache.is_linear());
    assert!(ServiceType::Receipt.is_linear());
    assert!(!ServiceType::Deps.is_linear());
    assert!(!Operation::opcode_inventory().iter().any(|name| name.starts_with("region.")));
    let incomplete = registry.lookup(&Operation::Core(CoreOp::ObjNew { declaration: sample(), type_args: vec![], fields: vec![v(1)], packet: Some(v(2)), construction: ConstructionStatus::Incomplete })).unwrap();
    assert!(incomplete.normal.postconditions.contains(&Postcondition::IncompleteIdentity));
    assert!(!incomplete.normal.postconditions.iter().any(|fact| matches!(fact, Postcondition::FreshAdmission(_))));
    let complete = registry.lookup(&Operation::Core(CoreOp::ObjInit { object: v(1), field: sample(), value: v(2), completion: Some(ConstructionCompletion { state: v(3) }) })).unwrap();
    assert!(complete.normal.transfers.contains(&TransferRule::CompleteIdentity { identity: v(1), state: v(3), output: SlotName::State }));
    let high = Operation::Exit(ExitOp::Prepare { exit: PreparedExit { packet: None, deps: None, ..sample() } });
    let contract = registry.lookup(&high).unwrap();
    assert!(contract.normal.transfers.is_empty());
    assert!(contract.normal.postconditions.iter().any(|fact| matches!(fact, Postcondition::LexicalExit { .. })));
    assert_eq!(operation_uses(&high, &body()).unwrap().len(), 2); // value and selection
    for boundary in [TakeBoundary::Existing { owner: sample() }, TakeBoundary::Temporary { awaiting: sample() }] {
        let contract = registry.lookup(&Operation::Async(AsyncOp::AwaitTake { future: v(1), result: Some(sample()), unique: v(2), boundary })).unwrap();
        assert_eq!(names(&contract.normal), [SlotName::Value, SlotName::Packet, SlotName::Deps]);
        assert_eq!(contract.normal.retention, [RetentionRule { result: SlotName::Value, selection: Retention::TakenOutcome { future: v(1), boundary } }]);
    }
    let shared = registry.lookup(&Operation::Async(AsyncOp::Await { future: v(1), result: Some(sample()) })).unwrap();
    assert_eq!(names(&shared.normal), [SlotName::Value, SlotName::Deps]);
    assert_eq!(shared.normal.retention[0].selection, Retention::Cache(v(1)));
    let shielded = registry.lookup(&Operation::Async(AsyncOp::AwaitShielded { future: v(1), result: None })).unwrap();
    assert!(shielded.cancellation.is_none());
    assert!(!shielded.effects.failures.contains(&FailureKind::Cancelled));
}

#[test]
fn call_entry_accept_and_reserved_scope_close_have_distinct_transitions() {
    let module = module(); let registry = ContractRegistry::new(&module);
    let accept = registry.lookup(&Operation::Ownership(OwnershipOp::Accept { state: v(1), packet: v(2) })).unwrap();
    assert_eq!(accept.normal.transfers, [TransferRule::Admit { state: v(1), packet: v(2), output: SlotName::State, provenance: ProvenanceTransition::Set(AdmissionProvenance::Bound) }]);
    assert_eq!(accept.operands.iter().map(|o| o.mode).collect::<Vec<_>>(), [OperandMode::Consume; 2]);
    for reserve in [None, Some(v(3))] {
        for active_failure in [false, true] {
            let op = if active_failure { OwnershipOp::ScopeUnwind { state: v(1), failure: v(2), reserve,
                effects: ScopeEffectEnvelope::conservative() } }
                else { OwnershipOp::ScopeEnd { state: v(1), reserve, effects: ScopeEffectEnvelope::conservative() } };
            let c = registry.lookup(&Operation::Ownership(op)).unwrap();
            let mut expected = vec![if active_failure { SlotName::Failure } else { SlotName::Closed }];
            if reserve.is_some() && !active_failure { expected.push(SlotName::Pending); }
            assert_eq!(names(&c.normal), expected);
            assert!(c.normal.postconditions.contains(&Postcondition::Closed(v(1))));
            if let Some(reserve) = reserve {
                assert!(c.normal.transfers.contains(&TransferRule::CloseReserve { scope: v(1), reserve,
                    output: if active_failure { None } else { Some(SlotName::Pending) } }));
                assert!(c.roots.roots.contains(&RootKind::PendingExit));
            }
            if let Some(unwind) = &c.unwind {
                let expected = vec![SlotName::Closed, SlotName::Failure];
                assert_eq!(names(unwind), expected);
                assert!(c.cancellation.is_none());
                if let Some(reserve) = reserve {
                    assert!(unwind.transfers.contains(&TransferRule::CloseReserve { scope: v(1), reserve,
                        output: None }));
                }
            } else { assert!(active_failure); }
        }
    }
    let missing = Operation::Call(CallOp::Direct { function: sample(), call: CallSite { summary: None, ..sample() } });
    assert_eq!(registry.lookup(&missing), Err(ContractError::MissingSummary));
}

#[test]
fn admissible_selection_preserves_ssa_leaves_and_detects_cycles() {
    let mut body = body();
    let admissible = body.selections.alloc(SelectionExpr::Admissible(sample())).unwrap();
    let op = Operation::Ownership(OwnershipOp::Split { packet: v(1), selection: admissible });
    assert_eq!(operation_uses(&op, &body).unwrap(), [OperandUse { value: v(1), mode: OperandMode::Consume }, OperandUse { value: v(0), mode: OperandMode::Borrow }]);
    *body.selections.get_mut(admissible).unwrap() = SelectionExpr::Admissible(admissible);
    assert_eq!(operation_uses(&op, &body), Err(TraversalError::CyclicExpression(OperandRef::Selection(admissible))));
}

#[test]
fn dynamic_census_add_merge_and_zero_counts() {
    let mut census = DynamicCensus::default(); let mut other = DynamicCensus::default();
    for reason in [DynamicReason::Atomic, DynamicReason::Cell, DynamicReason::Channel, DynamicReason::Group, DynamicReason::Any, DynamicReason::Closure, DynamicReason::Merge] {
        census.add(FuncId::from_u32(0), reason, 2);
        other.add(FuncId::from_u32(0), reason, 3);
        other.add(FuncId::from_u32(1), reason, 7);
    }
    census.merge(&other);
    for reason in [DynamicReason::Atomic, DynamicReason::Cell, DynamicReason::Channel, DynamicReason::Group, DynamicReason::Any, DynamicReason::Closure, DynamicReason::Merge] {
        assert_eq!(census.module_count(reason), 12);
        assert_eq!(census.function_count(FuncId::from_u32(0), reason), 5);
        assert_eq!(census.function_count(FuncId::from_u32(1), reason), 7);
        assert_eq!(census.function_count(FuncId::from_u32(2), reason), 0);
    }
    census.add(FuncId::from_u32(3), DynamicReason::Any, 0);
    census.merge(&DynamicCensus::default());
    assert_eq!(census.functions().count(), 2);
}

sample_expr!(ScopeEffectEnvelope, ScopeEffectEnvelope::conservative());
sample_expr!(FutureEnvironment, FutureEnvironment::Frame(sample()));

impl DeclaredOperands for ScopeEffectEnvelope {
    fn append_declared(&self, _: &mut Vec<OperandRef>) {}
}
impl DeclaredOperands for FutureEnvironment {
    fn append_declared(&self, refs: &mut Vec<OperandRef>) {
        match self {
            Self::Frame(frame) => frame.append_declared(refs),
            Self::Deferred { arguments, descriptors, .. } => {
                arguments.append_declared(refs);
                descriptors.append_declared(refs);
            }
        }
    }
}

#[test]
fn scope_envelope_projects_exact_outcome_availability_and_reserve_slots() {
    let module = module();
    let registry = ContractRegistry::new(&module);
    for reserve in [None, Some(v(2))] {
        for failures in [vec![], vec![FailureKind::Thrown]] {
            for cancel in [false, true] {
                let effects = ScopeEffectEnvelope { failures: failures.clone(), suspends: false, cancel };
                let end = registry.lookup(&Operation::Ownership(OwnershipOp::ScopeEnd {
                    state: v(1), reserve, effects: effects.clone(),
                })).unwrap();
                let expected = if reserve.is_some() {
                    vec![SlotName::Closed, SlotName::Pending]
                } else {
                    vec![SlotName::Closed]
                };
                assert_eq!(names(&end.normal), expected);
                assert_eq!(end.unwind.is_some(), !failures.is_empty() || cancel);
                assert_eq!(end.cancellation.is_some(), cancel);
                if let Some(unwind) = &end.unwind {
                    assert_eq!(names(unwind), [SlotName::Closed, SlotName::Failure]);
                }
                let unwind = registry.lookup(&Operation::Ownership(OwnershipOp::ScopeUnwind {
                    state: v(1), failure: v(3), reserve, effects,
                })).unwrap();
                assert_eq!(names(&unwind.normal), [SlotName::Failure]);
                assert!(unwind.unwind.is_none());
                assert!(unwind.cancellation.is_none());
                assert!(!end.effects.suspends);
                assert!(end.roots.roots.contains(&RootKind::CleanupRemainder));
            }
        }
    }
    let extend = registry.lookup(&Operation::Exit(ExitOp::Extend { pending: v(1), packet: v(2) })).unwrap();
    assert_eq!(names(&extend.normal), [SlotName::State]);
    assert_eq!(extend.normal.results[0].ty, ResultType::Service(ServiceType::Pending));
    assert_eq!(extend.operands.iter().map(|operand| operand.mode).collect::<Vec<_>>(),
        [OperandMode::Consume, OperandMode::Consume]);
}
