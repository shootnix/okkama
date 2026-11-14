//! Mutable structural visitors. They rewrite references, never infer facts or
//! prove their validity. Arena keys are moved only by an explicit editor remap.
use crate::*;

pub trait VisitorMut {
    fn cleanup_close(&mut self, _: &mut CleanupCloseId) {}
    fn cleanup_plan(&mut self, _: &mut CleanupPlanId) {}
    fn cleanup_schedule(&mut self, _: &mut CleanupScheduleId) {}
    fn root_map(&mut self, _: &mut RootMapId) {}
    fn value(&mut self, _: &mut ValueId) {}
    fn instruction(&mut self, _: &mut InstId) {}
    fn block(&mut self, _: &mut BlockId) {}
    fn binding(&mut self, _: &mut BindingId) {}
    fn boundary(&mut self, _: &mut BoundaryId) {}
    fn dependency(&mut self, _: &mut DepExprId) {}
    fn key(&mut self, _: &mut KeyExprId) {}
    fn selection(&mut self, _: &mut SelectionId) {}
    fn creation_site(&mut self, _: &mut CreationSiteId) {}
    fn ty(&mut self, _: &mut TypeId) {}
    fn declaration(&mut self, _: &mut DeclId) {}
    fn function(&mut self, _: &mut FuncId) {}
    fn scope(&mut self, _: &mut ScopeId) {}
    fn summary(&mut self, _: &mut SummaryId) {}
    fn protocol(&mut self, _: &mut ProtocolId) {}
    fn file(&mut self, _: &mut FileId) {}
    /// Called after visiting the file reference; may replace the whole location.
    fn location(&mut self, _: &mut DebugLoc) {}
}

pub trait Rewrite {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut);
}
macro_rules! leaf {
    ($($ty:ty => $method:ident),* $(,)?) => {$(
        impl Rewrite for $ty {
            fn rewrite(&mut self, visitor: &mut impl VisitorMut) { visitor.$method(self); }
        }
    )*};
}
leaf!(CleanupCloseId => cleanup_close, CleanupPlanId => cleanup_plan,
    CleanupScheduleId => cleanup_schedule, RootMapId => root_map, ValueId => value, InstId => instruction,
    BlockId => block, BindingId => binding,
    BoundaryId => boundary, DepExprId => dependency, KeyExprId => key,
    SelectionId => selection, CreationSiteId => creation_site, TypeId => ty,
    DeclId => declaration, FuncId => function, ScopeId => scope, SummaryId => summary,
    ProtocolId => protocol, FileId => file);
macro_rules! inert {
    ($($ty:ty),* $(,)?) => {$(impl Rewrite for $ty { fn rewrite(&mut self, _: &mut impl VisitorMut) {} })*};
}
inert!(MergeSource, u16, u32, bool, String, NumericType, ConstructionStatus, OperandMode,
    OperandConvention, DynamicReason, ExitKind, AbortPhase, DescriptorManaged,
    AdmissionProvenance, ServiceKeyKind, BoundaryKind, ScopeEffectEnvelope, CallCleanupMode,
    HighEffectEnvelope, HighEdgeSelector, DeferReturnTag);
impl<T: Rewrite> Rewrite for Vec<T> {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { for item in self { item.rewrite(visitor); } }
}
impl<T: Rewrite> Rewrite for Option<T> {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { if let Some(item) = self { item.rewrite(visitor); } }
}
impl<T: Rewrite> Rewrite for Box<T> {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { (**self).rewrite(visitor); }
}
impl<A: Rewrite, B: Rewrite> Rewrite for (A, B) {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { self.0.rewrite(visitor); self.1.rewrite(visitor); }
}
impl<I: ArenaId, T: Rewrite> Rewrite for Arena<I, T> {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { for (_, item) in self.iter_mut() { item.rewrite(visitor); } }
}
macro_rules! fields {
    ($($ty:ty { $($field:ident),* $(,)? }),* $(,)?) => {$(
        impl Rewrite for $ty {
            fn rewrite(&mut self, visitor: &mut impl VisitorMut) { $(self.$field.rewrite(visitor);)* }
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
impl Rewrite for DebugLoc {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { self.file.rewrite(visitor); visitor.location(self); }
}
impl Rewrite for Literal {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        if let Self::Enum { decl, .. } = self { decl.rewrite(visitor); }
    }
}
impl Rewrite for Operation {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::Const(op) => op.rewrite(visitor), Self::Core(op) => op.rewrite(visitor),
            Self::Lexical(op) => op.rewrite(visitor), Self::Ownership(op) => op.rewrite(visitor),
            Self::Call(op) => op.rewrite(visitor), Self::Collection(op) => op.rewrite(visitor),
            Self::Async(op) => op.rewrite(visitor), Self::Carrier(op) => op.rewrite(visitor),
            Self::Exit(op) => op.rewrite(visitor), Self::Cleanup(op) => op.rewrite(visitor),
        }
    }
}
impl Rewrite for Definition {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::BlockParam { block, .. } => block.rewrite(visitor),
            Self::InstResult { inst, .. } => inst.rewrite(visitor),
        }
    }
}
impl Rewrite for EdgeArg {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) { if let Self::Value(value) = self { value.rewrite(visitor); } }
}
impl Rewrite for TermKind {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::CleanupNext { cleanup, selected, done } => {
                cleanup.rewrite(visitor);
                selected.rewrite(visitor);
                done.rewrite(visitor);
            }
            Self::ExitDispatch { close, closed, failure, exit, success, pending, failed } => {
                close.rewrite(visitor);
                closed.rewrite(visitor);
                failure.rewrite(visitor);
                exit.rewrite(visitor);
                success.rewrite(visitor);
                pending.rewrite(visitor);
                failed.rewrite(visitor);
            }
            Self::FailureDispatch { failure, none, some } => {
                failure.rewrite(visitor);
                none.rewrite(visitor);
                some.rewrite(visitor);
            }

            Self::Br(target) => target.rewrite(visitor),
            Self::Cond { condition, yes, no } => { condition.rewrite(visitor); yes.rewrite(visitor); no.rewrite(visitor); }
            Self::Switch { value, cases, default } => { value.rewrite(visitor); cases.rewrite(visitor); default.rewrite(visitor); }
            Self::InvokeOwn { op, normal, unwind } => { op.rewrite(visitor); normal.rewrite(visitor); unwind.rewrite(visitor); }
            Self::InvokeDefer { function, call, continue_edge, replacements, unwind } => {
                function.rewrite(visitor);
                call.rewrite(visitor);
                continue_edge.rewrite(visitor);
                replacements.rewrite(visitor);
                unwind.rewrite(visitor);
            }
            Self::RetDefer { values, .. } => values.rewrite(visitor),
            Self::Ret { values } => values.rewrite(visitor),
            Self::Throw(value) | Self::Raise(value) | Self::Panic(value) => value.rewrite(visitor),
            Self::Catch { failure, ty, handler, next } => { failure.rewrite(visitor); ty.rewrite(visitor); handler.rewrite(visitor); next.rewrite(visitor); }
            Self::Lookup { collection, key, found, missing, .. } => { collection.rewrite(visitor); key.rewrite(visitor); found.rewrite(visitor); missing.rewrite(visitor); }
            Self::Trap { .. } | Self::Unreachable => {},
        }
    }
}
impl Rewrite for TransferStates {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Same(value) => value.rewrite(visitor), Self::Between { from, to } => { from.rewrite(visitor); to.rewrite(visitor); } }
    }
}
impl Rewrite for CreationOrigin {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Instruction(inst) => inst.rewrite(visitor), Self::Invoke { block, .. } => block.rewrite(visitor) }
    }
}
impl Rewrite for TakeBoundary {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Existing { owner } => owner.rewrite(visitor), Self::Temporary { awaiting } => awaiting.rewrite(visitor) }
    }
}
impl Rewrite for CarrierDestination {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::FailureContext(scope) => scope.rewrite(visitor), Self::Cache(value) | Self::Diagnostic(value) => value.rewrite(visitor) }
    }
}
impl Rewrite for ExitDestination {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Caller => {}, Self::Loop(boundary) => boundary.rewrite(visitor), Self::Scope(scope) => scope.rewrite(visitor) }
    }
}
impl Rewrite for SummarySelection {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Path { fields, .. } => fields.rewrite(visitor),
            Self::Field { field, .. } => field.rewrite(visitor), Self::Union(items) => items.rewrite(visitor),
            Self::Parameter(_) | Self::OwnerGraph(_) | Self::Descriptor(_) => {} }
    }
}
impl Rewrite for PlainCertificate {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Scalar => {}, Self::ClosedGraph { ty } => ty.rewrite(visitor), Self::Descriptor(value) => value.rewrite(visitor) }
    }
}
impl Rewrite for ValueFacts {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Plain { certificate } => certificate.rewrite(visitor), Self::Managed { deps } => deps.rewrite(visitor),
            Self::Unelaborated | Self::Service => {} }
    }
}
impl Rewrite for KeyExpr {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Created { site, instance } => { site.rewrite(visitor); instance.rewrite(visitor); }
            Self::ServiceOf { value, .. } | Self::Runtime(value) => value.rewrite(visitor) }
    }
}
impl Rewrite for DepExpr {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Empty => {}, Self::Key(key) => key.rewrite(visitor), Self::Union(items) => items.rewrite(visitor),
            Self::FromSsa(value) => value.rewrite(visitor), Self::Retention { value, selection } => { value.rewrite(visitor); selection.rewrite(visitor); } }
    }
}
impl Rewrite for SelectionExpr {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Value(value) | Self::OwnerGraph(value) | Self::ViewOwner(value) => value.rewrite(visitor),
            Self::Admissible(selection) => selection.rewrite(visitor),
            Self::Projection(projection) => projection.rewrite(visitor), Self::Union(items) => items.rewrite(visitor),
            Self::Descriptor { descriptor, value } => { descriptor.rewrite(visitor); value.rewrite(visitor); } }
    }
}
impl Rewrite for Projection {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self { Self::Field { value, field } => { value.rewrite(visitor); field.rewrite(visitor); }
            Self::Index { value, index } => { value.rewrite(visitor); index.rewrite(visitor); }
            Self::Slice { value, start, end } => { value.rewrite(visitor); start.rewrite(visitor); end.rewrite(visitor); }
            Self::Map { value, keys } => { value.rewrite(visitor); keys.rewrite(visitor); } }
    }
}

/// Visit every use (including unused declarations and facts), without changing
/// SSA definitions, block parameter/result lists or arena keys.
pub fn rewrite_uses(body: &mut Body, visitor: &mut impl VisitorMut) {
    body.cleanup.rewrite(visitor);
    body.high_results.rewrite(visitor);
    body.high_effects.rewrite(visitor);
    body.descriptor_bindings.rewrite(visitor);
    body.service_schedules.rewrite(visitor);
    body.keys.rewrite(visitor); body.deps.rewrite(visitor); body.selections.rewrite(visitor);
    for (_, data) in body.values.iter_mut() { data.facts.rewrite(visitor); }
    for (_, inst) in body.insts.iter_mut() { inst.op.rewrite(visitor); inst.facts.rewrite(visitor); inst.loc.rewrite(visitor); }
    body.bindings.rewrite(visitor);
    body.boundaries.rewrite(visitor);
    for (_, block) in body.blocks.iter_mut() { block.unwind.rewrite(visitor); block.term.rewrite(visitor); }
}

impl Rewrite for FutureEnvironment {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::Deferred { function, arguments, descriptors } => {
                function.rewrite(visitor);
                arguments.rewrite(visitor);
                descriptors.rewrite(visitor);
            }
            Self::Frame(frame) => frame.rewrite(visitor),
        }
    }
}

impl Rewrite for HighOperationSite {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::Instruction(instruction) => instruction.rewrite(visitor),
            Self::Terminator(block) => block.rewrite(visitor),
        }
    }
}

fields!(
    CleanupMetadata { closes, schedules, plans, roots },
    CleanupClose { source, after_defers, effects }, CleanupSchedule { close, holders },
    CleanupPlan { close, sources }, RootRequirement { boundary, bindings }, RootBinding { value, trace }
);
impl Rewrite for CleanupSource {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::Scope(value) => value.rewrite(visitor),
            Self::Packet(value) => value.rewrite(visitor),
        }
    }
}
impl Rewrite for RootBoundary {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::BeforeInstruction(value) => value.rewrite(visitor),
            Self::AfterInstruction(value) => value.rewrite(visitor),
            Self::Terminator(value) => value.rewrite(visitor),
            Self::Entry(value) => value.rewrite(visitor),
        }
    }
}
impl Rewrite for RootTrace {
    fn rewrite(&mut self, visitor: &mut impl VisitorMut) {
        match self {
            Self::Value(value) => value.rewrite(visitor),
            Self::Descriptor(value) => value.rewrite(visitor),
            _ => {},
        }
    }
}
