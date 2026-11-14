use super::*;

impl Engine<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cleanup_operation(
        &mut self, block: BlockId, op: &Operation, outcome: Outcome, before: &Flow, flow: &mut Flow,
        schema: &OutcomeContract, slots: &mut [Slot],
    ) {
        if let Operation::Ownership(OwnershipOp::ScopeEnd { state, reserve, effects }
            | OwnershipOp::ScopeUnwind { state, reserve, effects, .. }) = op {
            self.scope_effects(block, op, *state, *reserve, effects, before);
        }
        if outcome != Outcome::Normal {
            return;
        }
        if let Operation::Exit(ExitOp::Abort { pending, cleanup, phase }) = op {
            let mut token = before.tokens[cleanup.as_u32() as usize].clone();
            if let Some(fact) = &mut token.cleanup {
                let after = !fact.executed.is_empty();
                if fact.pending || after != (*phase == AbortPhase::AfterExecutedPrefix) {
                    self.error(R::B101,
                        "abort must preserve the executed prefix and cannot duplicate an owned reserve");
                }
                let incoming = &before.tokens[pending.as_u32() as usize].pieces;
                if !fact.shield || fact.awaiting_failure
                    || incoming.iter().any(|piece| fact.executed.contains(&piece.origin)
                        || token.pieces.iter().any(|old| old.overlaps(piece))) {
                    self.error(R::B101, "abort requires shielded disjoint reserve and a settled executed prefix");
                }
                if before.active_failure == 0 && fact.failures == 0 {
                    self.error(R::B101, "exit.abort requires an actual failure or delivered cancellation");
                }
                if fact.failures == 0 {
                    fact.failures = before.active_failure;
                    fact.failure_origin = before.active_failure_origin;
                }
                token.pieces.extend(incoming.iter().cloned());
                fact.inventory.extend(incoming.iter().cloned());
                fact.phase = 1;
                fact.plan = None;
            } else { self.error(R::B101, "exit.abort needs a live cleanup automaton"); }
            if let Some(slot) = slots.first_mut() {
                slot.token = token;
            }
            return;
        }
        let Operation::Cleanup(op) = op else {
            return;
        };
        if let CleanupOp::Begin { state, pending, failure, schedule } = op {
            let mut token = before.tokens[state.as_u32() as usize].clone();
            let scope = token.scope;
            let matching = self.body.service_schedules.iter().find(|entry| {
                Some(entry.owner) == scope && entry.dictionary == Some(*schedule)
                    && self.module.protocols.get(entry.protocol)
                        .is_ok_and(|protocol| protocol.kind == ProtocolKind::Services)
            });
            if matching.is_none() {
                self.error(R::B106, "cleanup.begin needs a typed service schedule for its owner");
            }
            let reserve = pending.map(|v| before.tokens[v.as_u32() as usize].pieces.clone()).unwrap_or_default();
            let failures = failure.map_or(0, |v| before.failures[v.as_u32() as usize]);
            let failure_origin = failure.and_then(|value| before.tokens[value.as_u32() as usize].failure_origin);
            if before.active_failure_origin.is_some() && before.active_failure_origin != failure_origin {
                self.error(R::B104, "cleanup.begin dropped or substituted the active primary failure/Error");
            }
            let mut fact = CleanupFact {
                phase: 0, shield: false, pending: pending.is_some(),
                inventory: token.pieces.union(&reserve).cloned().collect(), reserve,
                failures, failure_origin, cancel_delivered: false, may_fail: false,
                awaiting_failure: false, awaiting_source: None, awaiting_origin: None, selected: None,
                executed: BTreeSet::new(), services: BTreeSet::new(), had_services: false,
                schedule: *schedule, plan: None, test: None,
            };
            for (value, _) in self.body.values.iter() {
                if before.availability[value.as_u32() as usize] == AVAILABLE
                    && matches!(super::super::later::value_type(self.module, self.body, value),
                        Some(Type::Frame(_) | Type::Service(ServiceType::TaskState | ServiceType::Pin
                            | ServiceType::Receipt | ServiceType::CacheState)))
                    && let Some(key) = before.tokens[value.as_u32() as usize].service {
                    fact.services.insert(key);
                }
            }
            fact.had_services = !fact.services.is_empty();
            if let Some(schedule) = matching {
                let declared: BTreeSet<_> = schedule.duties.iter().filter_map(|value| {
                    before.tokens[value.as_u32() as usize].service
                }).collect();
                if declared != fact.services || declared.len() != schedule.duties.len() {
                    self.error(R::B106, "service schedule must cover every assigned holder exactly once");
                }
            }
            if failures != 0 {
                token.pieces.extend(fact.reserve.iter().cloned());
                fact.reserve.clear();
                fact.pending = false;
            }
            token.cleanup = Some(fact);
            if let Some(pending) = pending {
                token.selection = before.tokens[pending.as_u32() as usize].selection;
                token.pending_type = before.tokens[pending.as_u32() as usize].pending_type;
                token.pending_kind = before.tokens[pending.as_u32() as usize].pending_kind;
                token.pending_boundary = before.tokens[pending.as_u32() as usize].pending_boundary;
                token.reserve = before.tokens[pending.as_u32() as usize].reserve.clone();
            }
            if let Some(slot) = slots.first_mut() {
                slot.token = token;
                if let Some(pending) = pending {
                    let index = pending.as_u32() as usize;
                    slot.refs = before.refs[index].clone();
                    slot.requirements = before.full_deps[index].clone();
                    slot.aliases = before.aliases[index].clone();
                    slot.identity = before.identities[index];
                }
            }
            return;
        }
        let cleanup = match op {
            CleanupOp::Defer { cleanup, .. } | CleanupOp::Settle { cleanup, .. }
            | CleanupOp::Plan { cleanup } | CleanupOp::PlanChecked { cleanup, .. }
            | CleanupOp::SettleEmpty { cleanup, .. } | CleanupOp::DestroySelected { cleanup, .. }
            | CleanupOp::Next { cleanup }
            | CleanupOp::DestroyDirect { cleanup, .. } | CleanupOp::RecordFailure { cleanup, .. }
            | CleanupOp::Shield { cleanup } | CleanupOp::DeliverCancel { cleanup }
            | CleanupOp::Finish { cleanup } => *cleanup,
            _ => return,
        };
        let mut token = before.tokens[cleanup.as_u32() as usize].clone();
        let Some(mut fact) = token.cleanup.take() else {
            self.error(R::B102, "cleanup step has no checked begin/phase state");
            return;
        };
        if !fact.shield && !matches!(op, CleanupOp::Shield { .. }) {
            self.error(R::B105, "cleanup execution, settlement and cancellation require a shield");
        }
        if fact.awaiting_failure && !matches!(op, CleanupOp::RecordFailure { .. }) {
            self.error(R::B104, "destroy failure state must be dispatched before the next cleanup step");
        }
        match op {
            CleanupOp::Defer { action, .. } => {
                let target = self.body.values.get(*action).ok().and_then(|value| match value.def {
                    Definition::InstResult { inst, .. } => self.body.insts.get(inst).ok(),
                    _ => None,
                }).and_then(|inst| match &inst.op {
                    Operation::Core(CoreOp::FnBind { function, .. } | CoreOp::FnClosure { function, .. }) => {
                        self.module.functions.get(*function).ok()
                    }
                    _ => None,
                }).and_then(|function| function.summary).and_then(|id| self.module.summaries.get(id).ok());
                if target.is_none_or(|summary| !summary.contract.failures.is_empty()
                    || summary.contract.suspends || summary.contract.async_destroy) {
                    self.error(R::B104,
                        "effectful or replacing defers require explicit shielded invoke.defer outcomes");
                }
                if fact.phase != 0 {
                    self.error(R::B102, "defer cannot execute after service settlement");
                }
            }
            CleanupOp::Shield { .. } => fact.shield = true,
            CleanupOp::Settle { holder, protocol, .. } => {
                let kind = match super::super::later::value_type(self.module, self.body, *holder) {
                    Some(Type::Service(ServiceType::TaskState)) => HolderKind::Group,
                    Some(Type::Service(ServiceType::Receipt)) => HolderKind::Receipt,
                    Some(Type::Service(ServiceType::Pin)) => HolderKind::Pin,
                    Some(Type::Service(ServiceType::CacheState)) => HolderKind::Cache,
                    Some(Type::Frame(_)) => HolderKind::Frame,
                    _ => HolderKind::Scope,
                };
                if self.module.protocols.get(*protocol).ok().is_none_or(|protocol| !protocol.rules.iter().any(|rule| {
                    rule.transition == ProtocolTransition::Settle && rule.from == kind
                })) {
                    self.error(R::B106, "settlement protocol does not cover this holder kind");
                }
                if fact.phase > 1 {
                    self.error(R::B106, "service settlement must precede the serial plan");
                }
                if !before.tokens[holder.as_u32() as usize].service.is_some_and(|key| fact.services.remove(&key)) {
                    self.error(R::B106, "settlement is not one of this cleanup's outstanding service duties");
                }
                fact.may_fail = true;
                let incoming = &before.tokens[holder.as_u32() as usize];
                if incoming.frame.and_then(|key| before.frames.get(&key)).is_some_and(|layout| {
                    layout.values().any(|slot| slot.token.saved_type.is_some_and(|ty| {
                        super::super::contracts::linear(self.module, ty)
                    }))
                }) {
                    self.error(R::B95, "saved linear frame slots must enter cancellation before service settlement");
                }
                token.pieces.extend(incoming.pieces.iter().cloned());
                fact.inventory.extend(incoming.pieces.iter().cloned());
                fact.phase = 1;
            }
            CleanupOp::SettleEmpty { schedule, .. } => {
                if fact.phase > 1 || !fact.services.is_empty() || fact.schedule != *schedule {
                    self.error(R::B106, "empty settlement requires this owner's checked empty schedule");
                }
                fact.phase = 1;
            }
            CleanupOp::Plan { .. } | CleanupOp::PlanChecked { .. } => {
                if !fact.shield {
                    self.error(R::B105, "cleanup plan requires a shield");
                }
                if fact.phase != 1 || !fact.services.is_empty() {
                    self.error(R::B106, "every assigned service must settle before planning");
                }
                if let CleanupOp::PlanChecked { plan, .. } = op {
                    self.check_plan(before, cleanup, &token, &fact, *plan);
                    fact.plan = Some(*plan);
                } else {
                    self.error(R::B102, "cleanup.plan requires an explicit typed plan and roots");
                }
                fact.phase = 2;
            }
            CleanupOp::Next { .. } => {
                if fact.phase != 2 {
                    self.error(R::B102, "cleanup.next requires the common serial plan");
                }
                fact.selected = token.pieces.iter().next().map(|piece| piece.origin);
                fact.test = Some(cleanup);
                if let Some(slot) = slots.get_mut(2) {
                    slot.token.cleanup_test = Some(cleanup);
                }
                if let Some(slot) = slots.get_mut(1) {
                    slot.token.sources.insert(cleanup);
                    if let Some(selected) = fact.selected {
                        slot.refs.insert(selected);
                    }
                }
            }
            CleanupOp::DestroySelected { selected, .. } => {
                if !fact.shield || fact.phase != 2 || fact.selected.is_none()
                    || !before.refs[selected.as_u32() as usize].iter().any(|key| Some(*key) == fact.selected) {
                    self.error(R::B102, "runtime destroy needs the checked serial selector's selected key");
                }
                if let Some(key) = fact.selected.take() {
                    let family = token.pieces.iter().any(|piece| piece.origin == key && piece.family);
                    if !family {
                        token.pieces.retain(|piece| piece.origin != key);
                        flow.dead.insert(key);
                    }
                    fact.executed.insert(key);
                }
                fact.awaiting_failure = true;
                fact.awaiting_source = Some(cleanup);
            }
            CleanupOp::DestroyDirect { identity, .. } => {
                let key = self.identity(before, *identity);
                if !fact.shield {
                    self.error(R::B105, "destroy must execute shielded");
                }
                if fact.phase != 2 {
                    self.error(R::B102, "destroy precedes serial planning");
                }
                if fact.executed.contains(&key) || before.dead.contains(&key) {
                    self.error(R::B103, "destroy cannot reenter or destroy an already dead identity");
                }
                // Without an explicit order witness only a proved complete singleton is direct.
                let direct = fact.plan.and_then(|id| self.body.cleanup_plans.get(id).ok())
                    .is_some_and(|plan| !plan.merge && plan.entries.iter().any(|value| {
                        self.identity(before, *value) == key
                    }));
                if !direct || !token.pieces.iter().any(|piece| piece.origin == key)
                    || fact.plan.and_then(|id| self.body.cleanup_plans.get(id).ok()).is_some_and(|plan| {
                        plan.entries.iter().find(|value| !fact.executed.contains(&self.identity(before, **value)))
                            .is_none_or(|value| self.identity(before, *value) != key)
                    }) {
                    self.error(R::B102, "direct destroy needs complete singleton or explicit serial order evidence");
                }
                token.pieces.retain(|piece| piece.origin != key);
                fact.executed.insert(key);
                flow.dead.insert(key);
                fact.awaiting_failure = true;
                fact.awaiting_source = Some(cleanup);
            }
            CleanupOp::RecordFailure { failure, .. } => {
                let kind = before.failures[failure.as_u32() as usize];
                if fact.phase != 0 && (!fact.awaiting_failure || fact.awaiting_origin !=
                    before.tokens[failure.as_u32() as usize].failure_origin) {
                    self.error(R::B104, "cleanup.failure needs this step's current FailureState dispatch");
                }
                if fact.awaiting_source.is_some_and(|source| {
                    !before.tokens[failure.as_u32() as usize].sources.contains(&source)
                }) {
                    self.error(R::B104, "cleanup.failure substituted a different failure/Error view");
                }
                if kind == 0 {
                    self.error(R::B104, "cleanup.failure must record an actual failure view");
                }
                if fact.failures == 0 {
                    fact.failures = kind;
                    fact.failure_origin = before.tokens[failure.as_u32() as usize].failure_origin;
                }
                flow.active_failure = fact.failures;
                flow.active_failure_origin = fact.failure_origin;
                fact.awaiting_failure = false;
                fact.awaiting_source = None;
                fact.awaiting_origin = None;
                if !fact.reserve.is_empty() {
                    fact.phase = 1;
                    fact.plan = None;
                }
                token.pieces.extend(fact.reserve.iter().cloned());
                fact.reserve.clear();
                fact.pending = false;
            }
            CleanupOp::DeliverCancel { .. } => {
                let action = token.scope.and_then(|id| self.module.scopes.get(id).ok());
                if !self.declaration.attrs.contains(&FunctionAttr::Async)
                    || action.is_none_or(|scope| scope.kind != ScopeKind::Action || scope.parent.is_some()) {
                    self.error(R::B105, "cancellation is delivered only at the final async action exit");
                }
                if fact.failures == 0 {
                    fact.failures = failure_bit(FailureKind::Cancelled);
                    fact.failure_origin = match self.state.location {
                        L::Instruction { instruction, .. } => Some(Origin::ViewInstruction(instruction, u16::MAX)),
                        _ => None,
                    };
                }
                fact.cancel_delivered = true;
                flow.active_failure = fact.failures;
                flow.active_failure_origin = fact.failure_origin;
                if !fact.reserve.is_empty() {
                    fact.phase = 1;
                    fact.plan = None;
                }
                token.pieces.extend(fact.reserve.iter().cloned());
                fact.reserve.clear();
                fact.pending = false;
            }
            CleanupOp::Finish { .. } => {
                if fact.phase != 2 || !token.pieces.is_empty() || !fact.services.is_empty() {
                    self.error(R::B102, "cleanup.finish skips an unprocessed remainder or service phase");
                }
                let destroyed: Vec<_> = fact.inventory.difference(&fact.reserve).cloned().collect();
                self.invalidate(before, flow, &destroyed);
                flow.closed_inputs.extend(destroyed.iter().cloned());
                for piece in &destroyed {
                    if !fact.reserve.iter().any(|reserved| reserved.origin == piece.origin) {
                        flow.dead.insert(piece.origin);
                    }
                }
                if let Some(scope) = token.scope {
                    flow.scopes.insert(scope, 2);
                }
                let finish = match self.state.location {
                    L::Instruction { instruction, .. } => Some(Origin::ViewInstruction(instruction, u16::MAX)),
                    _ => None,
                };
                for slot in slots.iter_mut() {
                    slot.token.sources.insert(cleanup);
                    slot.token.finish_origin = finish;
                }
                for (info, slot) in schema.results.iter().zip(slots.iter_mut()) {
                    match info.ty {
                        ResultType::Service(ServiceType::FailureState) => {
                            slot.failures = if fact.failures != 0 { fact.failures }
                                else if fact.may_fail { 15 } else { 0 };
                            slot.token.failure_origin = fact.failure_origin.or({
                                if fact.may_fail {
                                    match self.state.location {
                                        L::Instruction { instruction, .. } => {
                                            Some(Origin::ViewInstruction(instruction, u16::MAX))
                                        }
                                        _ => None,
                                    }
                                } else { None }
                            });
                        }
                        ResultType::Service(ServiceType::ExitState) => {
                            slot.token.pieces = fact.reserve.clone();
                            slot.token.cleanup = Some(fact.clone());
                            slot.token.scope = token.scope;
                            slot.token.selection = token.selection;
                            slot.token.pending_type = token.pending_type;
                            slot.token.pending_kind = token.pending_kind;
                            slot.token.pending_boundary = token.pending_boundary;
                            slot.token.reserve = token.reserve.clone();
                            if let Some(scope) = token.scope {
                                slot.token.reserve.remove(&scope);
                            }
                            if fact.pending {
                                slot.refs = before.refs[cleanup.as_u32() as usize].clone();
                                slot.requirements = before.full_deps[cleanup.as_u32() as usize].clone();
                                slot.aliases = before.aliases[cleanup.as_u32() as usize].clone();
                                slot.identity = before.identities[cleanup.as_u32() as usize];
                            }
                        }
                        _ => {}
                    }
                }
                return;
            }
            _ => {}
        }
        if fact.awaiting_failure
            && let Some(slot) = slots.get_mut(1) {
            slot.token.sources.insert(cleanup);
            slot.token.failure_origin = Some(Origin::ViewInstruction(
                match self.state.location {
                    L::Instruction { instruction, .. } => instruction,
                    _ => InstId::from_u32(u32::MAX),
                }, u16::MAX));
            fact.awaiting_origin = slot.token.failure_origin;
            slot.failures = match op {
                CleanupOp::DestroyDirect { destroy, .. } => self.module.functions.get(*destroy).ok()
                    .and_then(|function| function.summary).and_then(|id| self.module.summaries.get(id).ok())
                    .map_or(15, |summary| summary.contract.failures.iter().filter(|kind| {
                        **kind != FailureKind::Cancelled
                    }).fold(0, |bits, kind| bits | failure_bit(*kind))),
                _ => 15,
            };
        }
        let pending = fact.pending;
        token.cleanup = Some(fact);
        if let Some(slot) = slots.first_mut() {
            slot.token = token;
            if pending {
                let index = cleanup.as_u32() as usize;
                slot.refs = before.refs[index].clone();
                slot.requirements = before.full_deps[index].clone();
                slot.aliases = before.aliases[index].clone();
                slot.identity = before.identities[index];
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scope_effects(
        &mut self, block: BlockId, op: &Operation, state: ValueId, reserve: Option<ValueId>,
        envelope: &ScopeEffectEnvelope, before: &Flow,
    ) {
        let pieces = before.tokens[state.as_u32() as usize].pieces.iter()
            .chain(reserve.into_iter().flat_map(|value| &before.tokens[value.as_u32() as usize].pieces))
            .cloned().collect();
        let required = self.required_effects(&pieces, before);
        if required.suspends && !envelope.suspends
            || required.failures.iter().any(|kind| !envelope.failures.contains(kind)) {
            self.error(R::B104, "scope effect envelope understates admitted or reserved cleanup effects");
        }
        if required.suspends && !self.declaration.attrs.contains(&FunctionAttr::Async)
            && !self.declaration.attrs.iter().any(|attr| {
                let FunctionAttr::Ctor(owner) = attr else {
                    return false;
                };
                self.module.declarations.get(*owner).is_ok_and(|declaration| {
                    if let DeclarationKind::Object { destroy: Some(destroy), .. } = declaration.kind {
                        self.module.functions.get(destroy).is_ok_and(|function| {
                            function.attrs.contains(&FunctionAttr::Async)
                        })
                    } else {
                        false
                    }
                })
            }) {
            self.error(R::B112, "synchronous owner contains an obligation with asynchronous cleanup");
        }
        if envelope.cancel {
            let scope = scope(self.module, self.body, state).and_then(|id| self.module.scopes.get(id).ok());
            let final_scope = scope.is_some_and(|scope| scope.parent.is_none() && scope.kind == ScopeKind::Action);
            let block_scope = self.body.blocks.get(block).ok().map(|block| block.lexical_scope);
            let active_children = before.scopes.iter().any(|(id, status)| *status == 1
                && Some(*id) != block_scope);
            if !self.declaration.attrs.contains(&FunctionAttr::Async) || !final_scope || active_children
                || before.active_failure != 0 || matches!(op, Operation::Ownership(OwnershipOp::ScopeUnwind { .. })) {
                self.error(R::B105, "cancel envelope is legal only at final successful async action close");
            }
        }
    }

    fn required_effects(&mut self, pieces: &BTreeSet<Piece>, before: &Flow) -> Effects {
        let mut required = Effects::default();
        for piece in pieces {
            let types = self.origin_types(piece.origin, piece, before);
            if types.is_empty() {
                required.suspends = true;
                required.failures = vec![FailureKind::Thrown, FailureKind::Program, FailureKind::Panic,
                    FailureKind::Native];
            }
            for ty in types {
                let effects = super::super::effects::cleanup(self.module, self.body, ty, self.state);
                required.suspends |= effects.suspends;
                for kind in effects.failures {
                    if !required.failures.contains(&kind) {
                        required.failures.push(kind);
                    }
                }
            }
        }
        required
    }

    fn origin_types(&self, origin: Origin, piece: &Piece, flow: &Flow) -> Vec<TypeId> {
        let values: Vec<_> = match origin {
            Origin::Input(value) | Origin::External(value) => vec![value],
            Origin::Instruction(inst, _) | Origin::RetiredInstruction(inst, _) => self.body.insts.get(inst)
                .map(|inst| inst.results.clone()).unwrap_or_default(),
            Origin::Invoke(block, slot) | Origin::RetiredInvoke(block, slot) => {
                if let Ok(block) = self.body.blocks.get(block)
                    && let TermKind::InvokeDefer { function, .. } = &block.term.kind {
                    let index = u32::MAX - slot;
                    if let Some(ty) = self.module.functions.get(*function).ok().and_then(|function| {
                        function.attrs.iter().find_map(|attr| if let FunctionAttr::DeferEntry { replacements } = attr {
                            replacements.get(index as usize).and_then(|label| label.result)
                        } else { None })
                    }) { return vec![ty]; }
                }
                self.body.blocks.get(block).ok()
                .and_then(|block| if let TermKind::InvokeOwn { normal, .. } = &block.term.kind {
                    self.body.blocks.get(normal.block).ok().map(|block| block.params.clone())
                } else { None }).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let mut types: Vec<_> = values.iter().filter_map(|value| self.body.values.get(*value).ok().map(|v| v.ty))
            .filter(|ty| !matches!(self.module.types.get(*ty), Ok(Type::Service(_)))).collect();
        if types.is_empty() {
            for filter in piece.predicates.keys() {
                for key in &filter.keys {
                    for (value, data) in self.body.values.iter() {
                        if flow.refs[value.as_u32() as usize].contains(key)
                            && !matches!(self.module.types.get(data.ty), Ok(Type::Service(_))) {
                            types.push(data.ty);
                        }
                    }
                }
            }
        }
        types
    }
}

impl Engine<'_, '_> {
    fn check_plan(&mut self, flow: &Flow, cleanup: ValueId, token: &Token, fact: &CleanupFact, id: CleanupPlanId) {
        let Ok(plan) = self.body.cleanup_plans.get(id) else {
            return;
        };
        if plan.effects.as_ref().is_some_and(|effects| effects.cancel) {
            let root = token.scope.and_then(|scope| self.module.scopes.get(scope).ok());
            if !self.declaration.attrs.contains(&FunctionAttr::Async)
                || root.is_none_or(|scope| scope.kind != ScopeKind::Action || scope.parent.is_some()) {
                self.error(R::B105, "lowered cancellation envelope requires the final async action boundary");
            }
        }
        if fact.cancel_delivered && plan.effects.as_ref().is_none_or(|effects| !effects.cancel) {
            self.error(R::B105, "lowered cancellation delivery is missing from its close envelope");
        }
        let mut effects = self.required_effects(&fact.inventory, flow);
        effects.suspends |= fact.had_services;
        if fact.had_services {
            for kind in [FailureKind::Thrown, FailureKind::Program, FailureKind::Panic, FailureKind::Native] {
                if !effects.failures.contains(&kind) {
                    effects.failures.push(kind);
                }
            }
        }
        if plan.effects.as_ref().is_none_or(|envelope| {
            effects.suspends && !envelope.suspends
                || effects.failures.iter().any(|kind| !envelope.failures.contains(kind))
        }) {
            self.error(R::B104, "lowered plan needs a sound close effect envelope including reserve and services");
        }
        if effects.suspends && !self.declaration.attrs.contains(&FunctionAttr::Async) {
            self.error(R::B112, "lowered synchronous cleanup owns an asynchronous or opaque source");
        }
        let keys: BTreeSet<_> = plan.entries.iter().map(|value| self.identity(flow, *value)).collect();
        let actual: BTreeSet<_> = token.pieces.iter().map(|piece| piece.origin).collect();
        if !keys.is_subset(&actual) || keys.len() != plan.entries.len()
            || plan.entries.iter().any(|value| flow.availability[value.as_u32() as usize] != AVAILABLE) {
            self.error(R::B102, "cleanup plan entries must be available distinct members of the live remainder");
        }
        if fact.may_fail && fact.pending && !plan.merge {
            self.error(R::B102, "service failure requires whole-reserve runtime merge before serial selection");
        }
        if !plan.merge && (keys != actual || keys.len() != plan.entries.len()
            || token.pieces.iter().any(|piece| piece.family || !piece.predicates.is_empty())) {
            self.error(R::B102, "static plan does not cover the complete singleton identities exactly once");
        }
        if !plan.merge && plan.entries.len() > 1 {
            let order = self.body.cleanup_orders.get(plan.order).ok();
            let mut positions = Vec::new();
            let mut matching = order.is_some_and(|order| order.creations.len() == plan.entries.len());
            if let Some(order) = order {
                for (creation, value) in order.creations.iter().zip(&plan.entries) {
                    if let Some((key, block, position)) = self.completion(flow, *creation) {
                        matching &= key == self.identity(flow, *value);
                        positions.push((block, position));
                    } else { matching = false; }
                }
            }
            if let Some(graph) = super::super::graph::Dominators::new(self.body, self.state) {
                for pair in positions.windows(2) {
                    let later = pair[0];
                    let earlier = pair[1];
                    if later.0 == earlier.0 {
                        matching &= later.1 > earlier.1;
                    }
                    else {
                        matching &= graph.dominates(super::super::graph::Dominators::entry(earlier.0) + 1,
                            super::super::graph::Dominators::entry(later.0), self.state);
                    }
                }
            } else { matching = false; }
            if !matching {
                self.error(R::B102, "direct order needs actual completion witnesses in descending serial order");
            }
        }
        let Ok(roots) = self.body.cleanup_roots.get(plan.roots) else {
            return;
        };
        let required = [RootKind::CleanupRemainder, RootKind::ActiveSelf, RootKind::ServiceSchedule];
        let suspends = plan.effects.as_ref().is_some_and(|effects| effects.suspends);
        let failure = fact.failures != 0 || plan.effects.as_ref().is_some_and(|effects| !effects.failures.is_empty());
        if suspends && !roots.through_suspend
            || failure && !roots.kinds.contains(&RootKind::CarrierGraph)
            || required.iter().any(|kind| !roots.kinds.contains(kind))
            || fact.pending && !roots.kinds.contains(&RootKind::PendingExit)
            || !roots.values.contains(&cleanup) {
            self.error(R::B105,
                "cleanup root map omits the active state, remainder, schedule, self or pending reserve");
        }
        for &value in &roots.values {
            if flow.availability[value.as_u32() as usize] != AVAILABLE
                || flow.lifetimes[value.as_u32() as usize] != 1
                || flow.dead.contains(&self.identity(flow, value)) {
                self.error(R::B105, "cleanup root map refers to an unavailable SSA holder");
            }
        }
    }

    fn completion(&self, flow: &Flow, creation: CreationOrigin) -> Option<(Origin, BlockId, usize)> {
        let (op, key, block, position) = match creation {
            CreationOrigin::Instruction(inst) => {
                let data = self.body.insts.get(inst).ok()?;
                let &(block, position) = self.state.positions.get(&(self.function, inst))?;
                (&data.op, Origin::Instruction(inst, 0), block, position)
            }
            CreationOrigin::Invoke { block, outcome: Outcome::Normal, slot: 0 } => {
                let TermKind::InvokeOwn { op, .. } = &self.body.blocks.get(block).ok()?.term.kind else {
                    return None;
                };
                (&**op, Origin::Invoke(block, 0), block, usize::MAX)
            }
            _ => return None,
        };
        match op {
            Operation::Core(CoreOp::ObjNew { construction: ConstructionStatus::Complete, .. }) => {
                Some((key, block, position))
            }
            Operation::Core(CoreOp::ObjInit { object, completion: Some(_), .. }) => {
                Some((self.identity(flow, *object), block, position))
            }
            _ => None,
        }
    }

    pub(super) fn dispatch(
        &mut self, flow: &mut Flow, closed: Option<ValueId>, failure: ValueId, exit: Option<ValueId>,
        arms: &[DispatchEdge], outgoing: &mut Vec<(BlockId, Flow)>,
    ) {
        let before = flow.clone();
        self.live_use(&before, failure);
        if let Some(exit) = exit {
            self.use_value(flow, exit, OperandMode::Consume, R::B101);
        }
        let fact = exit.and_then(|exit| before.tokens[exit.as_u32() as usize].cleanup.clone());
        if let (Some(exit), Some(closed)) = (exit, closed) {
            let sources = &before.tokens[exit.as_u32() as usize].sources;
            let generation = before.tokens[exit.as_u32() as usize].finish_origin;
            if sources.is_empty() || generation.is_none()
                || generation != before.tokens[closed.as_u32() as usize].finish_origin
                || generation != before.tokens[failure.as_u32() as usize].finish_origin {
                self.error(R::B104, "closed/failure/exit states must come from the same finish operation");
            }
        }
        if exit.is_some() && fact.is_none() {
            self.error(R::B101, "exit.dispatch requires the states produced together by cleanup.finish");
        }
        let mut expected = Vec::new();
        if let Some(fact) = &fact {
            if fact.failures != 0 {
                expected.push(DispatchKind::Failure);
            }
            else {
                if fact.pending {
                    expected.push(DispatchKind::Pending);
                }
                else { expected.push(DispatchKind::Success); }
                if fact.may_fail {
                    expected.push(DispatchKind::Failure);
                }
            }
        } else {
            expected.push(DispatchKind::None);
            if before.failures[failure.as_u32() as usize] != 0 {
                expected.push(DispatchKind::Some);
            }
        }
        if expected.iter().any(|kind| !arms.iter().any(|arm| arm.kind == *kind)) {
            self.error(R::B104, "typed dispatch omits a possible cleanup outcome");
        }
        for arm in arms {
            if exit.is_some() && !expected.contains(&arm.kind) {
                continue;
            }
            if exit.is_none() && arm.kind == DispatchKind::Some
                && before.failures[failure.as_u32() as usize] == 0 { continue; }
            let mut branch = flow.clone();
            if exit.is_some() && matches!(arm.kind, DispatchKind::Success | DispatchKind::Pending) {
                let generation = before.tokens[failure.as_u32() as usize].finish_origin;
                for (index, token) in branch.tokens.iter_mut().enumerate() {
                    if token.finish_origin == generation {
                        branch.failures[index] = 0;
                        token.failure_origin = None;
                    }
                }
            }
            if arm.kind == DispatchKind::Failure && let Some(fact) = &fact {
                let reserve: Vec<_> = fact.reserve.iter().cloned().collect();
                self.invalidate(&before, &mut branch, &reserve);
                branch.closed_inputs.extend(reserve.iter().cloned());
                branch.dead.extend(reserve.iter().map(|piece| piece.origin));
            }
            let mut slots = vec![Slot::default(); arm.kind.results().len()];
            if matches!(arm.kind, DispatchKind::Some | DispatchKind::Failure) {
                if branch.active_failure == 0 || exit.is_some() {
                    branch.active_failure = before.failures[failure.as_u32() as usize];
                    branch.active_failure_origin = before.tokens[failure.as_u32() as usize].failure_origin;
                }
                if let Some(slot) = slots.last_mut() {
                    slot.failures = before.failures[failure.as_u32() as usize];
                    slot.token = before.tokens[failure.as_u32() as usize].clone();
                }
            }
            if let Some(fact) = &fact {
                if arm.kind == DispatchKind::Pending
                && let (Some(slot), Some(exit)) = (slots.last_mut(), exit) {
                    slot.token = before.tokens[exit.as_u32() as usize].clone();
                    slot.token.cleanup = None;
                    slot.token.pieces = fact.reserve.clone();
                    let index = exit.as_u32() as usize;
                    slot.refs = before.refs[index].clone();
                    slot.requirements = before.full_deps[index].clone();
                    slot.aliases = before.aliases[index].clone();
                    slot.identity = before.identities[index];
                }
            } else {
                // Copyable FailureState is not a linear permission. Its association
                // with the current cleanup token prevents skipping a failed step.
                for token in &mut branch.tokens {
                    if let Some(fact) = &mut token.cleanup
                        && fact.awaiting_failure && arm.kind == DispatchKind::None
                            && fact.awaiting_source.is_some_and(|source| {
                                before.tokens[failure.as_u32() as usize].sources.contains(&source)
                            }) && fact.awaiting_origin == before.tokens[failure.as_u32() as usize].failure_origin {
                        fact.awaiting_failure = false;
                        fact.awaiting_source = None;
                        fact.awaiting_origin = None;
                    }
                }
            }
            let _ = closed;
            outgoing.push(self.edge(&branch, arm.edge.block, &arm.edge.args, &slots, R::B101));
        }
    }
}
