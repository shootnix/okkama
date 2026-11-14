use super::{read::{Read, Reader}, RuleId, State};
use crate::*;

pub(super) struct References<'a, 's> {
    pub module: &'a Module,
    pub body: Option<&'a Body>,
    pub function: Option<FuncId>,
    pub state: &'s mut State<'a>,
    pub valid: bool,
}
impl References<'_, '_> {
    fn invalid(&mut self, rule: RuleId, message: &str) {
        self.valid = false;
        self.state.error(rule, message);
    }
}
macro_rules! module_ref {
    ($($method:ident: $ty:ty => $arena:ident),* $(,)?) => {$(
        fn $method(&mut self, id: &$ty) {
            if !self.state.step(1) { return; }
            if self.module.$arena.get(*id).is_err() {
                self.invalid(RuleId::S1, concat!("invalid ", stringify!($method), " reference"));
            }
        }
    )*};
}
macro_rules! body_ref {
    ($($method:ident: $ty:ty => $arena:ident),* $(,)?) => {$(
        fn $method(&mut self, id: &$ty) {
            if !self.state.step(1) { return; }
            if self.body.is_none_or(|body| body.$arena.get(*id).is_err()) {
                self.invalid(RuleId::S3, concat!("invalid ", stringify!($method), " reference"));
            }
        }
    )*};
}
impl Reader for References<'_, '_> {
    fn active(&self) -> bool { !self.state.incomplete }
    fn depth(&mut self, depth: usize) {
        if depth > self.state.options.limits.expression_depth { self.state.incomplete = true; }
    }
    fn sequence(&mut self, len: usize, size: usize) {
        // This also reserves a conservative allowance for temporary registry
        // vectors before lookup or graph construction allocates them.
        self.state.entities(len);
        if self.state.step(len) {
            match size.checked_add(64) {
                Some(size) => { self.state.array(len, size); }
                None => self.state.incomplete = true,
            }
        }
    }
    module_ref!(ty: TypeId => types, declaration: DeclId => declarations,
        function: FuncId => functions, file: FileId => files);
    body_ref!(value: ValueId => values, instruction: InstId => insts,
        block: BlockId => blocks, binding: BindingId => bindings,
        boundary: BoundaryId => boundaries, dependency: DepExprId => deps,
        key: KeyExprId => keys, selection: SelectionId => selections);
    fn cleanup_close(&mut self, id: &CleanupCloseId) {
        if !self.state.step(1) { return; }
        if self.body.is_none_or(|body| body.cleanup.closes.get(*id).is_err()) {
            self.invalid(RuleId::S3, "invalid cleanup close reference");
        }
    }
    fn cleanup_order_proof(&mut self, id: &CleanupOrderProofId) {
        if !self.state.step(1) {
            return;
        }
        if self.body.is_none_or(|body| body.cleanup.proofs.get(*id).is_err()) {
            self.invalid(RuleId::S3, "invalid cleanup order proof reference");
        }
    }
    fn cleanup_plan(&mut self, id: &CleanupPlanId) {
        if !self.state.step(1) { return; }
        if self.body.is_none_or(|body| body.cleanup.plans.get(*id).is_err()) {
            self.invalid(RuleId::S3, "invalid cleanup plan reference");
        }
    }
    fn cleanup_schedule(&mut self, id: &CleanupScheduleId) {
        if !self.state.step(1) { return; }
        if self.body.is_none_or(|body| body.cleanup.schedules.get(*id).is_err()) {
            self.invalid(RuleId::S3, "invalid cleanup schedule reference");
        }
    }
    fn scope(&mut self, id: &ScopeId) {
        if !self.state.step(1) { return; }
        match self.module.scopes.get(*id) {
            Ok(scope) if self.function.is_none_or(|function| function == scope.function) => {}
            _ => self.invalid(RuleId::S2, "scope is absent or belongs to another function"),
        }
    }
    fn summary(&mut self, id: &SummaryId) {
        if !self.state.step(1) { return; }
        let Ok(summary) = self.module.summaries.get(*id) else {
            self.invalid(RuleId::S1, "invalid summary reference");
            return;
        };
        if summary.contract.results.len() > self.state.options.limits.outcome_slots {
            self.state.incomplete = true;
            return;
        }
        self.state.array(summary.parameters.len(), 128);
        self.state.array(summary.contract.results.len(), 256);
        self.state.step(summary.parameters.len());
        if let Some(protocol) = summary.contract.protocol { self.protocol(&protocol); }
    }
    fn protocol(&mut self, id: &ProtocolId) {
        if !self.state.step(1) { return; }
        let Ok(protocol) = self.module.protocols.get(*id) else {
            self.invalid(RuleId::S1, "invalid protocol reference");
            return;
        };
        if !self.state.array(protocol.rules.len(), 1024) { return; }
        self.state.step(protocol.rules.len());
        for rule in &protocol.rules {
            if !self.state.step(rule.checks.len()) { return; }
        }
    }
    fn location(&mut self, loc: &DebugLoc) {
        if loc.line == 0 || loc.column == 0 {
            self.invalid(RuleId::S1, "source line and column must be one-based");
        }
    }
}

pub(super) fn references<'a>(
    module: &'a Module,
    body: Option<&'a Body>,
    function: Option<FuncId>,
    item: &impl Read,
    state: &mut State<'a>,
) -> bool {
    let mut reader = References { module, body, function, state, valid: true };
    item.read(&mut reader);
    reader.valid && !reader.state.incomplete
}

/// Prerequisite check used before typing: a damaged reference already has a
/// structural diagnostic and must not produce dependent type errors.
pub(super) fn intact(module: &Module, body: &Body, item: &impl Read, state: &mut State<'_>) -> bool {
    struct Integrity<'m, 's, 'o> {
        module: &'m Module,
        body: &'m Body,
        valid: bool,
        state: &'s mut State<'o>,
    }
    macro_rules! check {
        ($($method:ident: $ty:ty => $owner:ident.$arena:ident),* $(,)?) => {$(
            fn $method(&mut self, id: &$ty) {
                if !self.state.step(1) {
                    self.valid = false;
                    return;
                }
                self.valid &= self.$owner.$arena.get(*id).is_ok();
            }
        )*};
    }
    impl Reader for Integrity<'_, '_, '_> {
        fn active(&self) -> bool { self.valid && !self.state.incomplete }
        fn sequence(&mut self, len: usize, _: usize) { self.valid &= self.state.step(len); }
        check!(instruction: InstId => body.insts, block: BlockId => body.blocks,
            binding: BindingId => body.bindings, boundary: BoundaryId => body.boundaries,
            dependency: DepExprId => body.deps, key: KeyExprId => body.keys,
            selection: SelectionId => body.selections, ty: TypeId => module.types,
            declaration: DeclId => module.declarations, function: FuncId => module.functions,
            scope: ScopeId => module.scopes, summary: SummaryId => module.summaries,
            protocol: ProtocolId => module.protocols, file: FileId => module.files);
        fn value(&mut self, id: &ValueId) {
            self.valid &= self.state.step(1) && self.body.values.get(*id).is_ok_and(|value| {
                self.module.types.get(value.ty).is_ok()
            });
        }
    }
    let mut reader = Integrity { module, body, valid: true, state };
    item.read(&mut reader);
    reader.valid && !reader.state.incomplete
}
