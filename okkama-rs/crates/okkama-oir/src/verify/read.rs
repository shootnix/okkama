//! Read-only reference traversal; expression DAGs are followed separately.
use crate::*;

pub trait Reader {
    fn active(&self) -> bool { true }
    fn sequence(&mut self, _: usize, _: usize) {}
    fn depth(&mut self, _: usize) {}
    fn cleanup_close(&mut self, _: &CleanupCloseId) {}
    fn cleanup_order_proof(&mut self, _: &CleanupOrderProofId) {}
    fn cleanup_plan(&mut self, _: &CleanupPlanId) {}
    fn cleanup_schedule(&mut self, _: &CleanupScheduleId) {}
    fn root_map(&mut self, _: &RootMapId) {}
    fn value(&mut self, _: &ValueId) {}
    fn instruction(&mut self, _: &InstId) {}
    fn block(&mut self, _: &BlockId) {}
    fn binding(&mut self, _: &BindingId) {}
    fn boundary(&mut self, _: &BoundaryId) {}
    fn dependency(&mut self, _: &DepExprId) {}
    fn key(&mut self, _: &KeyExprId) {}
    fn selection(&mut self, _: &SelectionId) {}
    fn creation_site(&mut self, _: &CreationSiteId) {}
    fn ty(&mut self, _: &TypeId) {}
    fn declaration(&mut self, _: &DeclId) {}
    fn function(&mut self, _: &FuncId) {}
    fn scope(&mut self, _: &ScopeId) {}
    fn summary(&mut self, _: &SummaryId) {}
    fn protocol(&mut self, _: &ProtocolId) {}
    fn file(&mut self, _: &FileId) {}
    fn location(&mut self, _: &DebugLoc) {}
}

pub trait Read {
    fn read(&self, visitor: &mut impl Reader);
}
macro_rules! leaf {
    ($($ty:ty => $method:ident),* $(,)?) => {$(
        impl Read for $ty {
            fn read(&self, visitor: &mut impl Reader) { visitor.$method(self); }
        }
    )*};
}
leaf!(CleanupOrderProofId => cleanup_order_proof, CleanupCloseId => cleanup_close, CleanupPlanId => cleanup_plan,
    CleanupScheduleId => cleanup_schedule, RootMapId => root_map, ValueId => value, InstId => instruction,
    BlockId => block, BindingId => binding,
    BoundaryId => boundary, DepExprId => dependency, KeyExprId => key,
    SelectionId => selection, CreationSiteId => creation_site, TypeId => ty,
    DeclId => declaration, FuncId => function, ScopeId => scope, SummaryId => summary,
    ProtocolId => protocol, FileId => file);
macro_rules! inert {
    ($($ty:ty),* $(,)?) => {$(impl Read for $ty { fn read(&self, _: &mut impl Reader) {} })*};
}
inert!(MergeSource, u16, u32, bool, NumericType, ConstructionStatus, OperandMode,
    OperandConvention, DynamicReason, ExitKind, AbortPhase, DescriptorManaged,
    AdmissionProvenance, ServiceKeyKind, BoundaryKind, ScopeEffectEnvelope, CallCleanupMode,
    HighEffectEnvelope, HighEdgeSelector, DeferReturnTag);
impl Read for String {
    fn read(&self, visitor: &mut impl Reader) { visitor.sequence(self.len(), 1); }
}
impl<T: Read> Read for Vec<T> {
    fn read(&self, visitor: &mut impl Reader) {
        visitor.sequence(self.len(), std::mem::size_of::<T>());
        for item in self {
            if !visitor.active() { break; }
            item.read(visitor);
        } }
}
impl<T: Read> Read for Option<T> {
    fn read(&self, visitor: &mut impl Reader) { if let Some(item) = self { item.read(visitor); } }
}
impl<T: Read> Read for Box<T> {
    fn read(&self, visitor: &mut impl Reader) { (**self).read(visitor); }
}
impl<A: Read, B: Read> Read for (A, B) {
    fn read(&self, visitor: &mut impl Reader) {
        self.0.read(visitor);
        self.1.read(visitor);
    }
}
impl<I: ArenaId, T: Read> Read for Arena<I, T> {
    fn read(&self, visitor: &mut impl Reader) { for (_, item) in self.iter() {
            if !visitor.active() { break; }
            item.read(visitor);
        } }
}
macro_rules! fields {
    ($($ty:ty { $($field:ident),* $(,)? }),* $(,)?) => {$(
        impl Read for $ty {
            fn read(&self, visitor: &mut impl Reader) { $(self.$field.read(visitor);)* }
        }
    )*};
}
fields!(
    DeferLabel { result }, DeferRegistration { boundary },
    DescriptorBinding { dictionary, contract }, ServiceSchedule { owner, protocol },
    HighResultExit { block, value, prepared }, HighEffectRecord { site },
    DeferReplaceEdge { edge },
    FieldRef { declaration }, FieldValue { field, value }, ConstructionCompletion { state },
    CallArgument { value }, CallSite { args, packet, deps, descriptors, type_args, summary },
    ResourceValue { value, packet }, AccessContext { owner, holder, authority },
    ElaboratedBinding { binding, resource, state, original_scope },
    ElaboratedStage { action, field, resource, states },
    DeferAction { function, captures, bindings, replacements },
    UniqueCacheProof { future, scope, region, evidence },
    UniqueCacheEvidence { creation, aliases }, FrameSlot { value }, SavedSlot { ty },
    DescriptorContract { retention, cleanup, summary },
    PreparedExit { value, packet, deps, selection, boundary },
    BindingDecl { scope, ty, loc }, Boundary { scope },
    Instruction { results, op, facts, loc }, ValueData { ty, def, facts },
    InstructionFacts { result_deps, retention, provenance }, ProvenanceFact { obligations },
    Target { block, args }, InvokeEdge { block, args }, BlockUnwind { target },
    Block { lexical_scope, params, unwind, insts, term },
    Terminator { kind, loc }, SwitchCase { value, target },
    Body { cleanup, entry, values, blocks, insts, bindings, boundaries, deps, keys, selections,
        high_results, high_effects, descriptor_bindings, service_schedules }
);
impl Read for DebugLoc {
    fn read(&self, visitor: &mut impl Reader) {
        self.file.read(visitor);
        visitor.location(self);
    }
}
impl Read for Literal {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Enum { decl, .. } => decl.read(visitor),
            Self::Int(value) | Self::Str(value) => value.read(visitor),
            Self::Rat { numerator, denominator } => {
                numerator.read(visitor);
                denominator.read(visitor);
            }
            Self::Float(_) | Self::Bool(_) | Self::Byte(_) | Self::Char(_) => {}
        }
    }
}
impl Read for Operation {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Const(op) => op.read(visitor), Self::Core(op) => op.read(visitor),
            Self::Lexical(op) => op.read(visitor), Self::Ownership(op) => op.read(visitor),
            Self::Call(op) => op.read(visitor), Self::Collection(op) => op.read(visitor),
            Self::Async(op) => op.read(visitor), Self::Carrier(op) => op.read(visitor),
            Self::Exit(op) => op.read(visitor), Self::Cleanup(op) => op.read(visitor),
        }
    }
}
impl Read for Definition {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::BlockParam { block, .. } => block.read(visitor),
            Self::InstResult { inst, .. } => inst.read(visitor),
        }
    }
}
impl Read for EdgeArg {
    fn read(&self, visitor: &mut impl Reader) { if let Self::Value(value) = self { value.read(visitor); } }
}
impl Read for TermKind {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::CleanupNext { cleanup, selected, done } => {
                cleanup.read(visitor);
                selected.read(visitor);
                done.read(visitor);
            }
            Self::ExitDispatch { close, closed, failure, exit, success, pending, failed } => {
                close.read(visitor);
                closed.read(visitor);
                failure.read(visitor);
                exit.read(visitor);
                success.read(visitor);
                pending.read(visitor);
                failed.read(visitor);
            }
            Self::FailureDispatch { failure, none, some } => {
                failure.read(visitor);
                none.read(visitor);
                some.read(visitor);
            }

            Self::Br(target) => target.read(visitor),
            Self::Cond { condition, yes, no } => {
                condition.read(visitor);
                yes.read(visitor);
                no.read(visitor);
            }
            Self::Switch { value, cases, default } => {
                value.read(visitor);
                cases.read(visitor);
                default.read(visitor);
            }
            Self::InvokeOwn { op, normal, unwind } => {
                op.read(visitor);
                normal.read(visitor);
                unwind.read(visitor);
            }
            Self::InvokeDefer { function, call, continue_edge, replacements, unwind } => {
                function.read(visitor);
                call.read(visitor);
                continue_edge.read(visitor);
                replacements.read(visitor);
                unwind.read(visitor);
            }
            Self::RetDefer { values, .. } => values.read(visitor),
            Self::Ret { values } => values.read(visitor),
            Self::Throw(value) | Self::Raise(value) | Self::Panic(value) => value.read(visitor),
            Self::Catch { failure, ty, handler, next } => {
                failure.read(visitor);
                ty.read(visitor);
                handler.read(visitor);
                next.read(visitor);
            }
            Self::Lookup { collection, key, found, missing, .. } => {
                collection.read(visitor);
                key.read(visitor);
                found.read(visitor);
                missing.read(visitor);
            }
            Self::Trap { .. } | Self::Unreachable => {}
        }
    }
}
impl Read for TransferStates {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Same(value) => value.read(visitor),
            Self::Between { from, to } => {
                from.read(visitor);
                to.read(visitor);
            }
        }
    }
}
impl Read for TakeBoundary {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Existing { owner } => owner.read(visitor),
            Self::Temporary { awaiting } => awaiting.read(visitor),
        }
    }
}
impl Read for CarrierDestination {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::FailureContext(scope) => scope.read(visitor),
            Self::Cache(value) | Self::Diagnostic(value) => value.read(visitor),
        }
    }
}
impl Read for ExitDestination {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Caller => {}
            Self::Loop(boundary) => boundary.read(visitor),
            Self::Scope(scope) => scope.read(visitor),
        }
    }
}
impl Read for SummarySelection {
    fn read(&self, visitor: &mut impl Reader) {
        let mut stack = vec![(self, 1_usize)];
        while let Some((item, depth)) = stack.pop() {
            if !visitor.active() { break; }
            visitor.depth(depth);
            if !visitor.active() { break; }
            match item {
                Self::Path { fields, .. } => fields.read(visitor),
            Self::Field { field, .. } => field.read(visitor),
                Self::Union(items) => {
                    visitor.sequence(items.len(), std::mem::size_of::<&Self>());
                    if visitor.active() {
                        stack.extend(items.iter().rev().map(|item| (item, depth.saturating_add(1))));
                    }
                }
                Self::Parameter(_) | Self::OwnerGraph(_) | Self::Descriptor(_) => {}
            }
        }
    }
}
impl Read for PlainCertificate {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Scalar => {}
            Self::ClosedGraph { ty } => ty.read(visitor),
            Self::Descriptor(value) => value.read(visitor),
        }
    }
}
impl Read for ValueFacts {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Plain { certificate } => certificate.read(visitor),
            Self::Managed { deps } => deps.read(visitor),
            Self::Unelaborated | Self::Service => {}
        }
    }
}
impl Read for KeyExpr {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Created { site, instance } => {
                site.read(visitor);
                instance.read(visitor);
            }
            Self::ServiceOf { value, .. } | Self::Runtime(value) => value.read(visitor),
        }
    }
}
impl Read for DepExpr {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Empty => {}
            Self::Key(key) => key.read(visitor),
            Self::Union(items) => items.read(visitor),
            Self::FromSsa(value) => value.read(visitor),
            Self::Retention { value, selection } => {
                value.read(visitor);
                selection.read(visitor);
            }
        }
    }
}
impl Read for SelectionExpr {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Value(value) | Self::OwnerGraph(value) | Self::ViewOwner(value) => value.read(visitor),
            Self::Projection(projection) => projection.read(visitor),
            Self::Admissible(selection) => selection.read(visitor),
            Self::Union(items) => items.read(visitor),
            Self::Descriptor { descriptor, value } => {
                descriptor.read(visitor);
                value.read(visitor);
            }
        }
    }
}
impl Read for Projection {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Field { value, field } => {
                value.read(visitor);
                field.read(visitor);
            }
            Self::Index { value, index } => {
                value.read(visitor);
                index.read(visitor);
            }
            Self::Slice { value, start, end } => {
                value.read(visitor);
                start.read(visitor);
                end.read(visitor);
            }
            Self::Map { value, keys } => {
                value.read(visitor);
                keys.read(visitor);
            }
        }
    }
}

impl Read for CreationOrigin {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Instruction(inst) => inst.read(visitor),
            Self::Invoke { block, .. } => block.read(visitor),
        }
    }
}

impl Read for FutureEnvironment {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Deferred { function, arguments, descriptors } => {
                function.read(visitor);
                arguments.read(visitor);
                descriptors.read(visitor);
            }
            Self::Frame(frame) => frame.read(visitor),
        }
    }
}

impl Read for HighOperationSite {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Instruction(instruction) => instruction.read(visitor),
            Self::Terminator(block) => block.read(visitor),
        }
    }
}

fields!(
    CleanupMetadata { closes, schedules, plans, roots, proofs },
    CleanupClose { source, after_defers, effects }, CleanupSchedule { close, holders },
    CleanupPlan { close, sources, direct }, RootRequirement { boundary, bindings }, RootBinding { value, trace }
);
impl Read for CleanupSource {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Scope(value) => value.read(visitor),
            Self::Packet(value) => value.read(visitor),
        }
    }
}
impl Read for RootBoundary {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::BeforeInstruction(value) => value.read(visitor),
            Self::AfterInstruction(value) => value.read(visitor),
            Self::Terminator(value) => value.read(visitor),
            Self::Entry(value) => value.read(visitor),
        }
    }
}
impl Read for RootTrace {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Value(value) => value.read(visitor),
            Self::Descriptor(value) => value.read(visitor),
            _ => {},
        }
    }
}

fields!(
    DirectSlot { identity, descriptor, destroy, origin, active_at },
    PreserveWitness { from, to }, JoinWitness { state, predecessors }, CleanupSuffix { completed, remaining, roots },
    CleanupOrderProof { state, slots, preserves, joins, suffixes, reserve_empty }
);
impl Read for CreationWitness {
    fn read(&self, visitor: &mut impl Reader) {
        match self {
            Self::Create { site } => site.read(visitor),
            Self::FreshResult { site, callee, summary } => {
                site.read(visitor);
                callee.read(visitor);
                summary.read(visitor);
            }
        }
    }
}
